//! 介面：上方篩選列、下方狀態列、四個虛擬化表格頁面。

use crate::app::{App, IconStore, Tab};
use crate::model::*;
use eframe::egui::{self, Color32, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

pub const DEFAULT_SORT: [(usize, bool); 4] = [(3, true), (3, true), (2, true), (3, true)];

// ---------- 上方列 ----------

pub fn top_bar(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (tab, label) in
                [(Tab::Comps, "熱門陣容"), (Tab::Units, "英雄"), (Tab::Items, "裝備"), (Tab::Traits, "羈絆")]
            {
                let text = RichText::new(label).size(16.0);
                ui.selectable_value(&mut app.tab, tab, text);
            }
            ui.separator();

            let mut f = app.filter;
            egui::ComboBox::from_id_salt("game")
                .selected_text(label_of(&GAME_TYPES, f.game_type))
                .show_ui(ui, |ui| {
                    for (v, l) in GAME_TYPES {
                        ui.selectable_value(&mut f.game_type, v, l);
                    }
                });
            egui::ComboBox::from_id_salt("rank")
                .selected_text(label_of(&RANK_GROUPS, f.rank_group))
                .show_ui(ui, |ui| {
                    for (v, l) in RANK_GROUPS {
                        ui.selectable_value(&mut f.rank_group, v, l);
                    }
                });
            egui::ComboBox::from_id_salt("patch")
                .selected_text(format!("版本 {}", patch_label(f.patch)))
                .show_ui(ui, |ui| {
                    for &p in &app.patches {
                        ui.selectable_value(&mut f.patch, p, patch_label(p));
                    }
                });
            app.set_filter(f);

            if app.is_loading() {
                ui.spinner();
            }
        });
        ui.add_space(2.0);
    });
}

fn label_of<T: PartialEq + Copy>(list: &[(T, &'static str)], v: T) -> &'static str {
    list.iter().find(|(k, _)| *k == v).map(|(_, l)| *l).unwrap_or("?")
}

// ---------- 狀態列 ----------

pub fn status_bar(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
            if let Some(s) = app.stats.get(&app.filter) {
                ui.label(format!("樣本 {} 場", fmt_int(s.total_entries)));
                if s.last_updated > 0 {
                    ui.label(format!("· 資料更新於 {}", ago(s.last_updated)));
                }
            }
            if app.offline {
                ui.colored_label(Color32::from_rgb(230, 160, 60), "· 離線：顯示快取資料");
            }
            if let Some(e) = &app.error {
                ui.colored_label(Color32::from_rgb(230, 90, 90), format!("· {e}"));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let t = &app.timings;
                let mut s = String::new();
                if let Some(ff) = t.first_frame {
                    s += &format!("首畫面 {} ms · 快取讀取 {:.1} ms", ff.as_millis(), t.cache_load.as_secs_f64() * 1000.0);
                }
                if let Some((what, d)) = &t.last_fetch {
                    s += &format!(" · {what}更新 {} ms", d.as_millis());
                }
                ui.weak(s);
                ui.weak("資料來源：tactics.tools / Community Dragon");
            });
        });
    });
}

fn ago(ts: u64) -> String {
    let d = now_secs().saturating_sub(ts);
    match d {
        0..=59 => "剛剛".into(),
        60..=3599 => format!("{} 分鐘前", d / 60),
        3600..=86399 => format!("{} 小時前", d / 3600),
        _ => format!("{} 天前", d / 86400),
    }
}

// ---------- 主畫面 ----------

pub fn central(app: &mut App, ui: &mut Ui) {
    toolbar(app, ui);
    ui.add_space(4.0);
    let id = app.tab as usize;
    ui.push_id(id, |ui| match app.tab {
        Tab::Comps => comps_tab(app, ui),
        Tab::Units => units_tab(app, ui),
        Tab::Items => items_tab(app, ui),
        Tab::Traits => traits_tab(app, ui),
    });
}

fn empty_state(app: &App, ui: &mut Ui) {
    ui.centered_and_justified(|ui| {
        if let Some(e) = &app.error {
            ui.colored_label(Color32::from_rgb(230, 90, 90), e);
        } else {
            ui.spinner();
        }
    });
}

/// 表格上方的工具列：左邊篩選、右邊搜尋（Ctrl+F 跳到搜尋框）。
fn toolbar(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
        match app.tab {
            Tab::Units => {
                ui.label(RichText::new("費用").strong());
                let mut cost = app.ui.cost;
                ui.selectable_value(&mut cost, None, "全部");
                for c in 1..=5u8 {
                    let t = RichText::new(format!("{c} 費")).color(cost_color(c)).strong();
                    ui.selectable_value(&mut cost, Some(c), t);
                }
                app.ui.cost = cost;
            }
            Tab::Items => {
                ui.label(RichText::new("類別").strong());
                let mut cat = app.ui.item_cat;
                ui.selectable_value(&mut cat, None, "全部");
                for (c, label) in ITEM_CATS {
                    ui.selectable_value(&mut cat, Some(c), label);
                }
                app.ui.item_cat = cat;
            }
            _ => {}
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let search = &mut app.ui.search[app.tab as usize];
            if !search.is_empty() && ui.button("清除").clicked() {
                search.clear();
            }
            let r = ui.add(
                egui::TextEdit::singleline(search)
                    .hint_text("🔍 搜尋（中文／英文，可模糊）")
                    .desired_width(260.0),
            );
            if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                r.request_focus();
            }
            if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                search.clear();
            }
        });
    });
}

/// 依欄位排序索引（只排索引，不搬資料）。
fn sorted<T>(rows: &[T], keep: impl Fn(&T) -> bool, key: impl Fn(&T) -> f64, asc: bool) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..rows.len()).filter(|&i| keep(&rows[i])).collect();
    idx.sort_by(|&a, &b| {
        let o = key(&rows[a]).total_cmp(&key(&rows[b]));
        if asc { o } else { o.reverse() }
    });
    idx
}

/// 可點擊排序的表頭。`default_asc`：第一次點時的方向（名次越低越好 → 遞增）。
fn sort_header(ui: &mut Ui, label: &str, col: usize, default_asc: bool, sort: &mut (usize, bool)) {
    let arrow = if sort.0 == col { if sort.1 { " ▲" } else { " ▼" } } else { "" };
    let r = ui.add(egui::Label::new(RichText::new(format!("{label}{arrow}")).strong()).sense(Sense::click()));
    if r.clicked() {
        if sort.0 == col {
            sort.1 = !sort.1;
        } else {
            *sort = (col, default_asc);
        }
    }
}

fn plain_header(ui: &mut Ui, label: &str) {
    ui.label(RichText::new(label).strong());
}

fn table<'a>(ui: &'a mut Ui, widths: &[f32]) -> TableBuilder<'a> {
    let mut t = TableBuilder::new(ui)
        .striped(true)
        .resizable(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
    for &w in widths {
        t = t.column(if w > 0.0 { Column::exact(w) } else { Column::remainder() });
    }
    t
}

// ---------- 熱門陣容 ----------

fn comps_tab(app: &mut App, ui: &mut Ui) {
    let Some(data) = app.comps.get(&app.filter) else { return empty_state(app, ui) };
    let st = &app.statics;
    let icons = &mut app.icons;
    let sort = &mut app.ui.sort[0];
    let search = app.ui.search[0].trim();

    // 場數太少的陣容名次不可信，依最熱門陣容的 5% 為門檻
    let min_count = data.comps.iter().map(|c| c.count).max().unwrap_or(0) / 20;
    let keep = |c: &CompRow| {
        if c.count < min_count {
            return false;
        }
        if search.is_empty() {
            return true;
        }
        let ids = c.carries.iter().chain(&c.core_units).chain(c.traits.iter().map(|(t, _)| t));
        let names: Vec<&str> = ids.flat_map(|id| [st.name(id), st.en(id)]).collect();
        crate::search::matches(search, &names)
    };
    let (col, asc) = *sort;
    let key = |c: &CompRow| match col {
        3 => c.place as f64,
        4 => c.top4 as f64,
        5 => c.win as f64,
        6 => c.count as f64,
        7 => c.lp_delta as f64,
        _ => c.place as f64,
    };
    let mut idx = sorted(&data.comps, keep, key, asc);
    crate::search::rank(&mut idx, search, |i| {
        let c = &data.comps[i];
        let ids = c.carries.iter().chain(&c.core_units).chain(c.traits.iter().map(|(t, _)| t));
        let names: Vec<&str> = ids.flat_map(|id| [st.name(id), st.en(id)]).collect();
        crate::search::score(search, &names).unwrap_or(0)
    });

    let mut clicked = None;
    table(ui, &[30.0, 0.0, 330.0, 80.0, 70.0, 70.0, 80.0, 70.0])
        .sense(Sense::click())
        .header(26.0, |mut h| {
            h.col(|ui| plain_header(ui, "#"));
            h.col(|ui| plain_header(ui, "陣容（點一下看詳細資料）"));
            h.col(|ui| plain_header(ui, "羈絆"));
            h.col(|ui| sort_header(ui, "平均名次", 3, true, sort));
            h.col(|ui| sort_header(ui, "前四%", 4, false, sort));
            h.col(|ui| sort_header(ui, "吃雞%", 5, false, sort));
            h.col(|ui| sort_header(ui, "場數", 6, false, sort));
            h.col(|ui| sort_header(ui, "LP", 7, false, sort));
        })
        .body(|body| {
            body.rows(62.0, idx.len(), |mut row| {
                let ci = idx[row.index()];
                let c = &data.comps[ci];
                let rank = row.index() + 1;
                row.col(|ui| {
                    ui.weak(rank.to_string());
                });
                row.col(|ui| {
                    ui.vertical(|ui| {
                        ui.add_space(2.0);
                        ui.label(RichText::new(crate::detail::comp_name(st, c)).strong());
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            for u in &c.carries {
                                unit_icon(ui, icons, st, u, 34.0);
                            }
                            ui.add_space(8.0);
                            for u in &c.core_units {
                                unit_icon(ui, icons, st, u, 26.0);
                            }
                        });
                    });
                });
                row.col(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (t, tier) in c.traits.iter().take(7) {
                        trait_chip(ui, icons, st, t, *tier);
                    }
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.2}", c.place)).color(place_color(c.place)).strong());
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", c.top4));
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", c.win));
                });
                row.col(|ui| {
                    ui.label(fmt_int(c.count as u64));
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:+.1}", c.lp_delta)).color(delta_color(c.lp_delta)));
                });
                if row.response().clicked() {
                    clicked = Some(ci);
                }
            });
        });
    if app.ui.open_first && !idx.is_empty() {
        app.ui.open_first = false;
        // 測試用：row=N 打開第 N 列（0 起算）
        let row = std::env::args()
            .find_map(|a| a.strip_prefix("row=").and_then(|n| n.parse::<usize>().ok()))
            .unwrap_or(0);
        clicked = Some(idx[row.min(idx.len() - 1)]);
    }
    if clicked.is_some() {
        app.ui.selected_comp = clicked;
    }
}

// ---------- 英雄 ----------

fn units_tab(app: &mut App, ui: &mut Ui) {
    let Some(data) = app.stats.get(&app.filter) else { return empty_state(app, ui) };
    let st = &app.statics;
    let icons = &mut app.icons;
    let sort = &mut app.ui.sort[1];
    let search = app.ui.search[1].trim();
    let cost = app.ui.cost;

    let keep = |u: &UnitRow| {
        cost.is_none_or(|c| st.cost(&u.id) == c) && crate::search::matches_id(search, st, &u.id)
    };
    let (col, asc) = *sort;
    let key = |u: &UnitRow| match col {
        2 => st.cost(&u.id) as f64,
        3 => u.place as f64,
        4 => u.top4 as f64,
        5 => u.won as f64,
        6 => u.count as f64,
        8 => u.star_count as f64,
        9 => if u.star_count > 0 { u.star_place as f64 } else { 9.0 },
        _ => u.place as f64,
    };
    let mut idx = sorted(&data.units, keep, key, asc);
    crate::search::rank(&mut idx, search, |i| crate::search::score_id(search, st, &data.units[i].id));

    let mut clicked: Option<String> = None;
    table(ui, &[30.0, 240.0, 50.0, 80.0, 70.0, 70.0, 90.0, 200.0, 80.0, 0.0])
        .sense(Sense::click())
        .header(26.0, |mut h| {
            h.col(|ui| plain_header(ui, "#"));
            h.col(|ui| plain_header(ui, "英雄（點一下看詳細資料）"));
            h.col(|ui| sort_header(ui, "費用", 2, true, sort));
            h.col(|ui| sort_header(ui, "平均名次", 3, true, sort));
            h.col(|ui| sort_header(ui, "前四%", 4, false, sort));
            h.col(|ui| sort_header(ui, "吃雞%", 5, false, sort));
            h.col(|ui| sort_header(ui, "場數", 6, false, sort));
            h.col(|ui| plain_header(ui, "常用裝備"));
            h.col(|ui| sort_header(ui, "三星場數", 8, false, sort));
            h.col(|ui| sort_header(ui, "三星名次", 9, true, sort));
        })
        .body(|body| {
            body.rows(40.0, idx.len(), |mut row| {
                let u = &data.units[idx[row.index()]];
                let rank = row.index() + 1;
                row.col(|ui| {
                    ui.weak(rank.to_string());
                });
                row.col(|ui| {
                    unit_icon(ui, icons, st, &u.id, 32.0);
                    ui.label(st.name(&u.id));
                });
                row.col(|ui| {
                    let c = st.cost(&u.id);
                    ui.label(RichText::new(c.to_string()).color(cost_color(c)));
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.2}", u.place)).color(place_color(u.place)).strong());
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", u.top4));
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", u.won));
                });
                row.col(|ui| {
                    ui.label(fmt_int(u.count as u64));
                });
                row.col(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for it in u.top_items.iter().take(5) {
                        item_icon(ui, icons, st, it, 28.0);
                    }
                });
                row.col(|ui| {
                    ui.label(fmt_int(u.star_count as u64));
                });
                row.col(|ui| {
                    if u.star_count > 0 {
                        ui.label(RichText::new(format!("{:.2}", u.star_place)).color(place_color(u.star_place)));
                    } else {
                        ui.weak("-");
                    }
                });
                if row.response().clicked() {
                    clicked = Some(u.id.clone());
                }
            });
        });
    if clicked.is_some() {
        app.ui.selected_unit = clicked;
    }
}

// ---------- 裝備 ----------

fn items_tab(app: &mut App, ui: &mut Ui) {
    let Some(data) = app.stats.get(&app.filter) else { return empty_state(app, ui) };
    let st = &app.statics;
    let icons = &mut app.icons;
    let sort = &mut app.ui.sort[2];
    let search = app.ui.search[2].trim();
    let cat = app.ui.item_cat;

    // 只藏掉極少見的（聖光、神化裝場數本來就少，門檻要低）
    let min_count = data.items.iter().map(|i| i.count).max().unwrap_or(0) / 1000;
    let keep = |i: &ItemRow| {
        i.count >= min_count
            && cat.is_none_or(|c| item_cat(&i.id) == c)
            && crate::search::matches_id(search, st, &i.id)
    };
    let (col, asc) = *sort;
    let key = |i: &ItemRow| match col {
        2 => i.place as f64,
        3 => i.top4 as f64,
        4 => i.won as f64,
        5 => i.count as f64,
        _ => i.place as f64,
    };
    let mut idx = sorted(&data.items, keep, key, asc);
    crate::search::rank(&mut idx, search, |i| crate::search::score_id(search, st, &data.items[i].id));

    let mut clicked: Option<String> = None;
    table(ui, &[30.0, 220.0, 80.0, 70.0, 70.0, 90.0, 0.0])
        .sense(Sense::click())
        .header(26.0, |mut h| {
            h.col(|ui| plain_header(ui, "#"));
            h.col(|ui| plain_header(ui, "裝備（點一下看詳細資料）"));
            h.col(|ui| sort_header(ui, "平均名次", 2, true, sort));
            h.col(|ui| sort_header(ui, "前四%", 3, false, sort));
            h.col(|ui| sort_header(ui, "吃雞%", 4, false, sort));
            h.col(|ui| sort_header(ui, "場數", 5, false, sort));
            h.col(|ui| plain_header(ui, "最適合的英雄"));
        })
        .body(|body| {
            body.rows(40.0, idx.len(), |mut row| {
                let i = &data.items[idx[row.index()]];
                let rank = row.index() + 1;
                row.col(|ui| {
                    ui.weak(rank.to_string());
                });
                row.col(|ui| {
                    item_icon(ui, icons, st, &i.id, 32.0);
                    ui.label(st.name(&i.id));
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.2}", i.place)).color(place_color(i.place)).strong());
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", i.top4));
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", i.won));
                });
                row.col(|ui| {
                    ui.label(fmt_int(i.count as u64));
                });
                row.col(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for (u, _) in i.top_users.iter().take(6) {
                        unit_icon(ui, icons, st, u, 30.0);
                    }
                });
                if row.response().clicked() {
                    clicked = Some(i.id.clone());
                }
            });
        });
    if clicked.is_some() {
        app.ui.selected_item = clicked;
    }
}

// ---------- 羈絆 ----------

fn traits_tab(app: &mut App, ui: &mut Ui) {
    let Some(data) = app.stats.get(&app.filter) else { return empty_state(app, ui) };
    let st = &app.statics;
    let icons = &mut app.icons;
    let sort = &mut app.ui.sort[3];
    let search = app.ui.search[3].trim();

    let keep = |t: &TraitRow| crate::search::matches_id(search, st, &t.id);
    let (col, asc) = *sort;
    let key = |t: &TraitRow| match col {
        2 => st.trait_units(&t.id, t.tier).unwrap_or(t.tier) as f64,
        3 => t.place as f64,
        4 => t.top4 as f64,
        5 => t.won as f64,
        6 => t.count as f64,
        _ => t.place as f64,
    };
    let mut idx = sorted(&data.traits, keep, key, asc);
    crate::search::rank(&mut idx, search, |i| crate::search::score_id(search, st, &data.traits[i].id));

    table(ui, &[30.0, 220.0, 60.0, 80.0, 70.0, 70.0, 0.0])
        .header(26.0, |mut h| {
            h.col(|ui| plain_header(ui, "#"));
            h.col(|ui| plain_header(ui, "羈絆"));
            h.col(|ui| sort_header(ui, "人數", 2, true, sort));
            h.col(|ui| sort_header(ui, "平均名次", 3, true, sort));
            h.col(|ui| sort_header(ui, "前四%", 4, false, sort));
            h.col(|ui| sort_header(ui, "吃雞%", 5, false, sort));
            h.col(|ui| sort_header(ui, "場數", 6, false, sort));
        })
        .body(|body| {
            body.rows(36.0, idx.len(), |mut row| {
                let t = &data.traits[idx[row.index()]];
                let rank = row.index() + 1;
                row.col(|ui| {
                    ui.weak(rank.to_string());
                });
                row.col(|ui| {
                    icon(ui, icons, st.icon(&t.id), 24.0, Some(tier_color(t.tier)));
                    ui.label(st.name(&t.id));
                });
                row.col(|ui| match st.trait_units(&t.id, t.tier) {
                    Some(n) => {
                        ui.label(n.to_string());
                    }
                    None => {
                        ui.weak(format!("階段 {}", t.tier));
                    }
                });
                row.col(|ui| {
                    ui.label(RichText::new(format!("{:.2}", t.place)).color(place_color(t.place)).strong());
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", t.top4));
                });
                row.col(|ui| {
                    ui.label(format!("{:.1}%", t.won));
                });
                row.col(|ui| {
                    ui.label(fmt_int(t.count as u64));
                });
            });
        });
}

// ---------- 小元件 ----------

/// 畫一個圖示；尚未載入時畫佔位方塊（不阻塞）。
pub(crate) fn icon(ui: &mut Ui, icons: &mut IconStore, url: Option<&str>, size: f32, border: Option<Color32>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let inner = match border {
            Some(c) => {
                painter.rect_filled(rect, 4.0, c);
                rect.shrink(2.0)
            }
            None => rect,
        };
        match url.and_then(|u| icons.get(u)) {
            Some(tex) => {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(tex.id(), inner, uv, Color32::WHITE);
            }
            None => {
                painter.rect_filled(inner, 3.0, ui.visuals().faint_bg_color);
            }
        }
    }
    resp
}

pub(crate) fn unit_icon(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, size: f32) {
    unit_icon_tip(ui, icons, st, id, size, st.name(id).to_string());
}

/// 英雄頭像，點了打開英雄詳細資料
pub(crate) fn unit_icon_tip(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, size: f32, tip: String) {
    let c = st.cost(id);
    let r = icon(ui, icons, st.icon(id), size, Some(cost_color(c))).on_hover_text(tip);
    if c > 0 {
        let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
        if r.clicked() {
            icons.nav = Some(crate::app::Nav::Unit(id.to_string()));
        }
    }
}

pub(crate) fn item_icon(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, size: f32) {
    item_icon_tip(ui, icons, st, id, size, st.name(id).to_string());
}

/// 裝備圖示，點了打開裝備詳細資料
pub(crate) fn item_icon_tip(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, size: f32, tip: String) {
    let r = icon(ui, icons, st.icon(id), size, None).on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
    if r.clicked() {
        icons.nav = Some(crate::app::Nav::Item(id.to_string()));
    }
}

fn trait_chip(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, tier: u8) {
    let n = st.trait_units(id, tier);
    let tip = match n {
        Some(n) => format!("{} {}", st.name(id), n),
        None => st.name(id).to_string(),
    };
    icon(ui, icons, st.icon(id), 22.0, Some(tier_color(tier))).on_hover_text(&tip);
    if let Some(n) = n {
        ui.label(RichText::new(n.to_string()).small());
    }
}

pub(crate) fn cost_color(c: u8) -> Color32 {
    match c {
        1 => Color32::from_rgb(140, 140, 140),
        2 => Color32::from_rgb(40, 170, 90),
        3 => Color32::from_rgb(50, 120, 220),
        4 => Color32::from_rgb(170, 70, 220),
        5 => Color32::from_rgb(230, 180, 40),
        _ => Color32::from_rgb(90, 90, 90),
    }
}

pub(crate) fn tier_color(t: u8) -> Color32 {
    match t {
        0 | 1 => Color32::from_rgb(150, 100, 60),
        2 => Color32::from_rgb(140, 150, 160),
        3 => Color32::from_rgb(220, 180, 60),
        _ => Color32::from_rgb(180, 120, 230),
    }
}

/// 平均名次越低越綠、越高越紅
pub(crate) fn place_color(p: f32) -> Color32 {
    let t = ((p - 3.8) / (5.0 - 3.8)).clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| (a + (b - a) * t) as u8;
    Color32::from_rgb(lerp(80.0, 235.0), lerp(210.0, 90.0), lerp(120.0, 90.0))
}

pub(crate) fn delta_color(d: f32) -> Color32 {
    if d >= 0.0 { Color32::from_rgb(90, 200, 120) } else { Color32::from_rgb(230, 100, 100) }
}

pub(crate) fn fmt_int(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
