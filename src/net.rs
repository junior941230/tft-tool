//! 網路抓取。全部在背景執行緒做，結果用 channel 丟回 UI 並喚醒重繪。

use crate::{cache, data, model::*};
use eframe::egui;
use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// 從 tactics.tools 前端程式碼取得的版本清單（新到舊）；更新的版本會在背景探測。
pub const KNOWN_PATCHES: [u32; 12] =
    [16230, 16220, 16212, 16210, 16201, 16200, 16191, 16190, 16181, 16180, 16173, 16170];

const STATICS_URL: &str = "https://raw.communitydragon.org/latest/cdragon/tft/zh_tw.json";
/// 靜態資料（中文名稱）多久重抓一次
pub const STATICS_MAX_AGE: u64 = 12 * 3600;
pub const ICON_SIZE: u32 = 64;

pub enum Msg {
    Stats(Filter, Result<StatsData, String>, Duration),
    Comps(Filter, Result<CompsData, String>, Duration),
    Statics(Result<Statics, String>, Duration),
    NewPatches(Vec<u32>),
    Entity(EntityKey, Result<EntityDetail, String>),
    Icon(String, Option<egui::ColorImage>),
}

fn client() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent("tft-tool/0.1")
            .gzip(true)
            .brotli(true)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(8)
            .build()
            .expect("HTTP client")
    })
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let resp = client().get(url).send().map_err(|e| {
        if e.is_connect() || e.is_timeout() { "無法連線（離線？）".to_string() } else { format!("網路錯誤：{e}") }
    })?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err("這個條件沒有資料".into());
    }
    if !status.is_success() {
        return Err(format!("伺服器回應 {status}"));
    }
    resp.bytes().map(|b| b.to_vec()).map_err(|e| format!("下載中斷：{e}"))
}

pub fn stats_url(f: &Filter) -> String {
    format!("https://d3.tft.tools/stats2/general/{}/{}/{}", f.game_type, f.patch, f.rank_group)
}

pub fn comps_url(f: &Filter) -> String {
    let path = if f.game_type >= 1160 { "duo-team-compositions" } else { "team-compositions" };
    format!("https://api.tft.tools/{path}/{}/{}", f.rank_group, f.patch)
}

pub fn entity_url(k: &EntityKey) -> String {
    let kind = if k.is_item { "item" } else { "unit" };
    let f = &k.filter;
    format!("https://d3.tft.tools/stats2/{kind}/{}/{}/{}/{}", f.game_type, k.id, f.patch, f.rank_group)
}

/// 背景抓單一英雄／裝備的詳細資料。
pub fn fetch_entity(k: EntityKey, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let r = get(&entity_url(&k)).and_then(|b| data::parse_entity(&b));
        if let Ok(d) = &r {
            cache::save(&cache::entity_key(&k), d);
        }
        notify(&tx, &ctx, Msg::Entity(k, r));
    });
}

fn notify(tx: &Sender<Msg>, ctx: &egui::Context, m: Msg) {
    let _ = tx.send(m);
    ctx.request_repaint();
}

/// 背景抓統計資料：下載 → 解析 → 寫快取 → 通知 UI。
pub fn fetch_stats(f: Filter, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let t = Instant::now();
        let r = get(&stats_url(&f)).and_then(|b| data::parse_stats(&b));
        if let Ok(d) = &r {
            cache::save(&cache::stats_key(&f), d);
        }
        notify(&tx, &ctx, Msg::Stats(f, r, t.elapsed()));
    });
}

pub fn fetch_comps(f: Filter, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let t = Instant::now();
        let r = get(&comps_url(&f)).and_then(|b| data::parse_comps(&b));
        if let Ok(d) = &r {
            cache::save(&cache::comps_key(&f), d);
        }
        notify(&tx, &ctx, Msg::Comps(f, r, t.elapsed()));
    });
}

pub fn fetch_statics(tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let t = Instant::now();
        let mut r = get(STATICS_URL).and_then(|b| data::parse_statics(&b));
        // 英文名稱（搜尋用）；抓不到也不影響中文
        if let Ok(st) = &mut r {
            if let Some(set) = data::latest_set(st) {
                if let Ok(js) = get(&format!("https://ap.tft.tools/static/s{set}/en.js")) {
                    data::apply_english(st, &String::from_utf8_lossy(&js));
                }
            }
        }
        if let Ok(d) = &r {
            cache::save("statics.bin", d);
        }
        notify(&tx, &ctx, Msg::Statics(r, t.elapsed()));
    });
}

/// 探測比目前最新更新的版本號（不存在的版本只會回 36 bytes 的 404）。
pub fn probe_patches(latest: u32, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let mut found = Vec::new();
        let mut base = latest;
        loop {
            let minor = base / 10 * 10;
            let cands = [base + 1, base + 2, minor + 10, minor + 11, minor + 20];
            let hits: Vec<u32> = std::thread::scope(|s| {
                let hs: Vec<_> = cands
                    .iter()
                    .map(|&p| {
                        s.spawn(move || {
                            let f = Filter { game_type: 1100, rank_group: 1, patch: p };
                            client()
                                .head(stats_url(&f))
                                .send()
                                .ok()
                                .filter(|r| r.status().is_success())
                                .map(|_| p)
                        })
                    })
                    .collect();
                hs.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
            });
            let Some(&max) = hits.iter().max() else { break };
            found.extend(hits);
            base = max;
        }
        if !found.is_empty() {
            found.sort_unstable_by(|a, b| b.cmp(a));
            found.dedup();
            notify(&tx, &ctx, Msg::NewPatches(found));
        }
    });
}

// ---------- 圖示 ----------

/// 固定數量的圖示工作執行緒；佇列是後進先出，讓目前畫面上剛要求的圖示先載入。
pub struct IconLoader {
    queue: Arc<(Mutex<VecDeque<String>>, Condvar)>,
}

impl IconLoader {
    pub fn new(tx: Sender<Msg>, ctx: egui::Context, workers: usize) -> Self {
        let queue: Arc<(Mutex<VecDeque<String>>, Condvar)> = Default::default();
        for _ in 0..workers {
            let q = queue.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let url = {
                    let (lock, cv) = &*q;
                    let mut g = lock.lock().unwrap();
                    loop {
                        if let Some(u) = g.pop_back() {
                            break u;
                        }
                        g = cv.wait(g).unwrap();
                    }
                };
                let img = load_icon(&url);
                notify(&tx, &ctx, Msg::Icon(url, img));
            });
        }
        Self { queue }
    }

    pub fn request(&self, url: String) {
        let (lock, cv) = &*self.queue;
        lock.lock().unwrap().push_back(url);
        cv.notify_one();
    }
}

fn load_icon(url: &str) -> Option<egui::ColorImage> {
    let path = cache::icon_path(url);
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(img) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
            return Some(to_color_image(img.to_rgba8()));
        }
    }
    let bytes = get(url).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    // 縮成顯示尺寸再存，下次直接讀小檔
    let small = if img.width() > ICON_SIZE || img.height() > ICON_SIZE {
        img.resize(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = small.to_rgba8();
    let _ = rgba.save_with_format(&path, image::ImageFormat::Png);
    Some(to_color_image(rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 連網測試：cargo test -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_endpoints() {
        let f = Filter { game_type: 1100, rank_group: 0, patch: KNOWN_PATCHES[0] };
        let t = Instant::now();
        let stats = data::parse_stats(&get(&stats_url(&f)).unwrap()).unwrap();
        println!("stats {:?}: {} units {} items {} traits", t.elapsed(), stats.units.len(), stats.items.len(), stats.traits.len());
        let t = Instant::now();
        let comps = data::parse_comps(&get(&comps_url(&f)).unwrap()).unwrap();
        println!("comps {:?}: {} comps", t.elapsed(), comps.comps.len());
        let t = Instant::now();
        let st = data::parse_statics(&get(STATICS_URL).unwrap()).unwrap();
        println!("statics {:?}: {} entries", t.elapsed(), st.map.len());

        assert_eq!(st.name("DA_18_Teemo"), "提摩");
        let u = &stats.units[0];
        println!("best unit: {} ({}) place {}", st.name(&u.id), u.id, u.place);
        let c = &comps.comps[0];
        println!(
            "best comp: place {:.2} top4 {:.1}% carries {:?} traits {:?}",
            c.place,
            c.top4,
            c.carries.iter().map(|x| st.name(x)).collect::<Vec<_>>(),
            c.traits.iter().map(|(t, n)| format!("{}{}", st.name(t), st.trait_units(t, *n).unwrap_or(0))).collect::<Vec<_>>()
        );
        for u in &stats.units {
            assert!(st.map.contains_key(&u.id), "缺中文名稱 {}", u.id);
        }
        // 快取來回
        let t = Instant::now();
        let bytes = bincode::serialize(&comps).unwrap();
        let _: CompsData = bincode::deserialize(&bytes).unwrap();
        println!("comps cache {} bytes, roundtrip {:?}", bytes.len(), t.elapsed());
        let url = st.icon("DA_18_Teemo").unwrap().to_string();
        assert!(load_icon(&url).is_some());
    }
}

fn to_color_image(rgba: image::RgbaImage) -> egui::ColorImage {
    let size = [rgba.width() as usize, rgba.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
}
