//! 應用程式狀態：記憶體快取 → 磁碟快取 → 網路（先顯示舊資料，背景更新）。

use crate::{cache, model::*, net, ui};
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Comps,
    Units,
    Items,
    Traits,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Stats,
    Comps,
}

/// 圖示：url → 材質。尚未載入時送進背景佇列。
pub struct IconStore {
    map: HashMap<String, Option<egui::TextureHandle>>,
    loader: net::IconLoader,
    /// 這一幀被點擊的英雄／裝備圖示（畫面最後再打開對應的詳細視窗）
    pub nav: Option<Nav>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Nav {
    Unit(String),
    Item(String),
}

impl IconStore {
    pub fn get(&mut self, url: &str) -> Option<&egui::TextureHandle> {
        if !self.map.contains_key(url) {
            self.map.insert(url.to_owned(), None);
            self.loader.request(url.to_owned());
        }
        self.map.get(url).and_then(|t| t.as_ref())
    }
}

#[derive(Default)]
pub struct Timings {
    pub first_frame: Option<Duration>,
    pub cache_load: Duration,
    pub last_fetch: Option<(String, Duration)>,
}

pub struct UiState {
    /// 每個頁籤各自的搜尋字串
    pub search: [String; 4],
    pub cost: Option<u8>,
    pub item_cat: Option<ItemCat>,
    /// 每個頁籤的 (排序欄位, 是否遞增)
    pub sort: [(usize, bool); 4],
    /// 目前打開詳細資料的陣容（data.comps 的索引）
    pub selected_comp: Option<usize>,
    pub detail_tab: u8,
    /// 啟動參數 comp：自動打開第一個陣容（測試用）
    pub open_first: bool,
    pub selected_unit: Option<String>,
    pub selected_item: Option<String>,
    pub unit_tab: u8,
    pub item_tab: u8,
}

pub struct App {
    t0: Instant,
    pub filter: Filter,
    pub patches: Vec<u32>,
    pub tab: Tab,
    pub statics: Statics,
    pub stats: HashMap<Filter, StatsData>,
    pub comps: HashMap<Filter, CompsData>,
    /// 英雄／裝備詳細資料
    pub entities: HashMap<EntityKey, EntityDetail>,
    ent_requested: HashSet<EntityKey>,
    ent_loader: net::EntityLoader,
    /// 已經排過預載的篩選條件
    prefetched: HashSet<Filter>,
    pub ent_pending: HashSet<EntityKey>,
    pub ent_errors: HashMap<EntityKey, String>,
    zoom: f32,
    /// 這次執行已經向網路要求過的（避免重複抓）
    requested: HashSet<(Kind, Filter)>,
    /// 正在下載中
    pending: HashSet<(Kind, Filter)>,
    statics_pending: bool,
    pub error: Option<String>,
    pub offline: bool,
    pub icons: IconStore,
    pub ui: UiState,
    pub timings: Timings,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
}

use net::Msg;

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, t0: Instant) -> Self {
        let ctx = cc.egui_ctx.clone();
        let (tx, rx) = channel();
        let tl = Instant::now();

        let filter: Filter = cache::load("filter.bin").unwrap_or_default();
        let mut patches: Vec<u32> = cache::load("patches.bin").unwrap_or_else(|| net::KNOWN_PATCHES.to_vec());
        merge_patches(&mut patches, &net::KNOWN_PATCHES);
        let statics: Statics = cache::load("statics.bin").unwrap_or_default();

        // 可用命令列參數直接開到某一頁：tft-tool.exe units|items|traits
        let tab = match std::env::args().nth(1).as_deref() {
            Some("units") => Tab::Units,
            Some("items") => Tab::Items,
            Some("traits") => Tab::Traits,
            _ => Tab::Comps,
        };
        let mut app = Self {
            t0,
            filter,
            patches,
            tab,
            statics,
            stats: HashMap::new(),
            comps: HashMap::new(),
            entities: HashMap::new(),
            ent_requested: HashSet::new(),
            ent_loader: net::EntityLoader::new(tx.clone(), ctx.clone(), 6),
            prefetched: HashSet::new(),
            ent_pending: HashSet::new(),
            ent_errors: HashMap::new(),
            zoom: cache::load("zoom.bin").unwrap_or(DEFAULT_ZOOM),
            requested: HashSet::new(),
            pending: HashSet::new(),
            statics_pending: false,
            error: None,
            offline: false,
            icons: IconStore { map: HashMap::new(), loader: net::IconLoader::new(tx.clone(), ctx.clone(), 8), nav: None },
            ui: UiState { search: Default::default(), cost: None, item_cat: None, sort: ui::DEFAULT_SORT, selected_comp: None, detail_tab: std::env::args().find_map(|a| a.strip_prefix("comp=").and_then(|n| n.parse().ok())).unwrap_or(0), open_first: std::env::args().any(|a| a.starts_with("comp")),
                // 測試用：unit=ID / item=ID 直接打開詳細資料
                selected_unit: arg_value("unit="),
                selected_item: arg_value("item="),
                unit_tab: arg_value("tab=").and_then(|t| t.parse().ok()).unwrap_or(0),
                item_tab: arg_value("tab=").and_then(|t| t.parse().ok()).unwrap_or(0),
            },
            timings: Timings::default(),
            tx,
            rx,
            ctx,
        };
        // 測試用：q=搜尋字、cost=費用、cat=radiant|artifact|emblem|normal
        if let Some(q) = arg_value("q=") {
            app.ui.search[app.tab as usize] = q;
        }
        app.ui.cost = arg_value("cost=").and_then(|c| c.parse().ok());
        app.ui.item_cat = arg_value("cat=").and_then(|c| match c.as_str() {
            "normal" => Some(ItemCat::Normal),
            "radiant" => Some(ItemCat::Radiant),
            "artifact" => Some(ItemCat::Artifact),
            "emblem" => Some(ItemCat::Emblem),
            _ => None,
        });
        app.ctx.set_zoom_factor(app.zoom);
        app.ensure_current();
        app.timings.cache_load = tl.elapsed();

        if now_secs().saturating_sub(app.statics.fetched_at) > net::STATICS_MAX_AGE {
            app.statics_pending = true;
            net::fetch_statics(app.tx.clone(), app.ctx.clone());
        }
        let latest = app.patches[0];
        net::probe_patches(latest, app.tx.clone(), app.ctx.clone());
        app
    }

    /// 確保目前篩選條件的資料：記憶體 → 磁碟 → 網路；也預抓其他牌位的小份統計資料。
    pub fn ensure_current(&mut self) {
        let f = self.filter;
        self.ensure(Kind::Stats, f);
        self.ensure(Kind::Comps, f);
        for &(rg, _) in RANK_GROUPS.iter() {
            if rg != f.rank_group {
                self.ensure(Kind::Stats, Filter { rank_group: rg, ..f });
            }
        }
    }

    fn ensure(&mut self, kind: Kind, f: Filter) {
        match kind {
            Kind::Stats if !self.stats.contains_key(&f) => {
                if let Some(d) = cache::load::<StatsData>(&cache::stats_key(&f)) {
                    self.stats.insert(f, d);
                }
            }
            Kind::Comps if !self.comps.contains_key(&f) => {
                if let Some(d) = cache::load::<CompsData>(&cache::comps_key(&f)) {
                    self.comps.insert(f, d);
                }
            }
            _ => {}
        }
        if self.requested.insert((kind, f)) {
            self.pending.insert((kind, f));
            match kind {
                Kind::Stats => net::fetch_stats(f, self.tx.clone(), self.ctx.clone()),
                Kind::Comps => net::fetch_comps(f, self.tx.clone(), self.ctx.clone()),
            }
        }
    }

    /// 英雄／裝備詳細資料：記憶體 → 磁碟 → 網路（每次執行每個條件只抓一次）。
    pub fn ensure_entity(&mut self, k: &EntityKey) {
        if !self.entities.contains_key(k) {
            if let Some(d) = cache::load::<EntityDetail>(&cache::entity_key(k)) {
                self.entities.insert(k.clone(), d);
            }
        }
        if self.ent_requested.insert(k.clone()) {
            self.ent_pending.insert(k.clone());
            self.ent_loader.push_front(k.clone());
        } else if !self.entities.contains_key(k) && self.ent_pending.contains(k) {
            // 還在預載佇列裡排隊：移到最前面
            self.ent_loader.push_front(k.clone());
        }
    }

    /// 統計資料到了以後，把這個條件下所有英雄和裝備的詳細資料排進背景佇列，
    /// 之後點開就不用等。
    fn prefetch_entities(&mut self) {
        let f = self.filter;
        let Some(stats) = self.stats.get(&f) else { return };
        if !self.prefetched.insert(f) {
            return;
        }
        let max_item = stats.items.iter().map(|i| i.count).max().unwrap_or(0);
        let mut units: Vec<_> = stats.units.iter().filter(|u| u.count > 0).collect();
        units.sort_by(|a, b| b.count.cmp(&a.count));
        let mut items: Vec<_> = stats.items.iter().filter(|i| i.count > 0 && i.count >= max_item / 1000).collect();
        items.sort_by(|a, b| b.count.cmp(&a.count));
        let keys: Vec<EntityKey> = units
            .iter()
            .map(|u| EntityKey { is_item: false, filter: f, id: u.id.clone() })
            .chain(items.iter().map(|i| EntityKey { is_item: true, filter: f, id: i.id.clone() }))
            .collect();
        // 磁碟快取不在這裡讀（一次讀幾百個檔會卡一下畫面），點開時 ensure_entity 會讀
        for k in keys {
            if self.ent_requested.insert(k.clone()) {
                self.ent_pending.insert(k.clone());
                self.ent_loader.push_back(k);
            }
        }
    }

    pub fn set_filter(&mut self, f: Filter) {
        if f != self.filter {
            self.filter = f;
            self.error = None;
            self.ui.selected_comp = None;
            cache::save("filter.bin", &f);
            self.ensure_current();
        }
    }

    pub fn is_loading(&self) -> bool {
        self.statics_pending
            || self.pending.contains(&(Kind::Stats, self.filter))
            || self.pending.contains(&(Kind::Comps, self.filter))
    }

    fn handle_messages(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Stats(f, r, dt) => {
                    self.pending.remove(&(Kind::Stats, f));
                    let r = r.map(|d| {
                        self.stats.insert(f, d);
                    });
                    self.on_result(f, "統計", dt, r);
                }
                Msg::Comps(f, r, dt) => {
                    self.pending.remove(&(Kind::Comps, f));
                    let r = r.map(|d| {
                        self.comps.insert(f, d);
                    });
                    self.on_result(f, "陣容", dt, r);
                }
                Msg::Statics(r, dt) => {
                    self.statics_pending = false;
                    match r {
                        Ok(s) => {
                            self.statics = s;
                            self.timings.last_fetch = Some(("中文資料".into(), dt));
                        }
                        Err(e) if self.statics.map.is_empty() => self.error = Some(format!("中文名稱：{e}")),
                        Err(_) => {}
                    }
                }
                Msg::NewPatches(ps) => {
                    merge_patches(&mut self.patches, &ps);
                    cache::save("patches.bin", &self.patches);
                    // 原本停在舊的最新版 → 自動跳到新的最新版
                    if ps.first().is_some_and(|&p| p > self.filter.patch)
                        && self.filter.patch == net::KNOWN_PATCHES[0]
                    {
                        let f = Filter { patch: self.patches[0], ..self.filter };
                        self.set_filter(f);
                    }
                }
                Msg::Entity(k, r) => {
                    self.ent_pending.remove(&k);
                    match r {
                        Ok(d) => {
                            self.ent_errors.remove(&k);
                            self.entities.insert(k, d);
                        }
                        Err(e) => {
                            self.ent_requested.remove(&k);
                            self.ent_errors.insert(k, e);
                        }
                    }
                }
                Msg::Icon(url, img) => {
                    let tex = img.map(|i| self.ctx.load_texture(&url, i, egui::TextureOptions::LINEAR));
                    self.icons.map.insert(url, tex);
                }
            }
        }
    }

    fn on_result(&mut self, f: Filter, what: &str, dt: Duration, r: Result<(), String>) {
        match r {
            Ok(()) => {
                if f == self.filter {
                    self.offline = false;
                    self.timings.last_fetch = Some((what.into(), dt));
                }
            }
            Err(e) => {
                if f == self.filter {
                    self.offline = e.contains("無法連線");
                    let has_cache = match what {
                        "統計" => self.stats.contains_key(&f),
                        _ => self.comps.contains_key(&f),
                    };
                    if !has_cache {
                        self.error = Some(format!("{what}：{e}"));
                    }
                }
                // 失敗的允許之後重試
                let kind = if what == "統計" { Kind::Stats } else { Kind::Comps };
                self.requested.remove(&(kind, f));
            }
        }
    }
}

/// 預設把整個介面放大 1.2 倍
pub const DEFAULT_ZOOM: f32 = 1.2;

fn arg_value(prefix: &str) -> Option<String> {
    std::env::args().find_map(|a| a.strip_prefix(prefix).map(String::from))
}

fn merge_patches(list: &mut Vec<u32>, add: &[u32]) {
    list.extend_from_slice(add);
    list.sort_unstable_by(|a, b| b.cmp(a));
    list.dedup();
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.timings.first_frame.is_none() {
            self.timings.first_frame = Some(self.t0.elapsed());
        }
        self.handle_messages();
        self.prefetch_entities();
        ui::top_bar(self, ctx);
        ui::status_bar(self, ctx);
        egui::CentralPanel::default().show(ctx, |ui| ui::central(self, ui));
        crate::detail::comp_detail(self, ctx);
        crate::entity::unit_window(self, ctx);
        crate::entity::item_window(self, ctx);

        // 點了某個英雄／裝備圖示 → 打開它的詳細資料
        match self.icons.nav.take() {
            Some(Nav::Unit(id)) => self.ui.selected_unit = Some(id),
            Some(Nav::Item(id)) => self.ui.selected_item = Some(id),
            None => {}
        }
        // 記住縮放（Ctrl + / Ctrl - 可調整）
        let z = ctx.zoom_factor();
        if (z - self.zoom).abs() > 0.001 {
            self.zoom = z;
            cache::save("zoom.bin", &z);
        }
    }
}
