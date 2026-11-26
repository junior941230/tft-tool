//! 陣容詳細資料視窗：總覽 / 變化陣容 / 羈絆 / 頂尖玩家。

use crate::app::{App, IconStore};
use crate::model::*;
use crate::ui::{cost_color, delta_color, fmt_int, icon, item_icon, place_color, tier_color, unit_icon};
use eframe::egui::{self, Color32, RichText, Sense, Ui};

pub(crate) const BAR_PURPLE: Color32 = Color32::from_rgb(120, 100, 210);
pub(crate) const BAR_TEAL: Color32 = Color32::from_rgb(40, 170, 140);
pub(crate) const BAR_RED: Color32 = Color32::from_rgb(200, 90, 90);

/// 陣容名稱：人數最多的羈絆 ＋ 前兩個主力英雄。
pub fn comp_name(st: &Statics, c: &CompRow) -> String {
    let top_trait = c
        .traits
        .iter()
        .max_by_key(|(t, tier)| st.trait_units(t, *tier).unwrap_or(0))
        .map(|(t, _)| st.name(t));
    let carries: Vec<&str> = c.carries.iter().take(2).map(|u| st.name(u)).collect();
    match top_trait {
        Some(t) => format!("{t} {}", carries.join(" & ")),
        None => carries.join(" & "),
    }
}

pub fn comp_detail(app: &mut App, ctx: &egui::Context) {
    let Some(ci) = app.ui.selected_comp else { return };
    let Some(c) = app.comps.get(&app.filter).and_then(|d| d.comps.get(ci)) else {
        app.ui.selected_comp = None;
        return;
    };
    let st = &app.statics;
    let icons = &mut app.icons;
    let tab = &mut app.ui.detail_tab;
    let d = &c.detail;
    let mut open = true;

    egui::Window::new(RichText::new(comp_name(st, c)).size(18.0).strong())
        .id(egui::Id::new("comp_detail"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([760.0, 680.0])
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, label) in ["總覽", "變化陣容", "羈絆", "頂尖玩家"].iter().enumerate() {
                    ui.selectable_value(tab, i as u8, RichText::new(*label).size(15.0));
                }
                if !d.code.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("複製陣容代碼").on_hover_text("貼到遊戲內的陣容規劃器").clicked() {
                            ui.ctx().copy_text(d.code.clone());
                        }
                    });
                }
            });
            ui.separator();
            match *tab {
                1 => variants(ui, icons, st, d),
                2 => traits(ui, icons, st, d),
                3 => players(ui, d),
                _ => overview(ui, icons, st, c),
            }
        });

    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.ui.selected_comp = None;
    }
}

// ---------- 小元件 ----------

pub(crate) fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(17.0).strong());
    ui.add_space(4.0);
}

pub(crate) fn stars(n: u8) -> RichText {
    let color = match n {
        3 => Color32::from_rgb(240, 200, 60),
        2 => Color32::from_rgb(200, 210, 220),
        _ => Color32::from_rgb(160, 110, 70),
    };
    RichText::new("★".repeat(n as usize)).color(color).size(11.0)
}

/// 英雄卡：星級 / 頭像 / 推薦裝備。
fn unit_card(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, star: u8, items: &[String], share: f32) {
    ui.vertical(|ui| {
        ui.set_width(50.0);
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(if star >= 2 { stars(star) } else { RichText::new(" ").size(11.0) });
        icon(ui, icons, st.icon(id), 48.0, Some(cost_color(st.cost(id))))
            .on_hover_text(format!("{}（出現率 {:.0}%）", st.name(id), share * 100.0));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            for it in items.iter().take(3) {
                item_icon(ui, icons, st, it, 15.0);
            }
        });
    });
}

/// 橫條：佔比 frac（0~1）＋右側文字
pub(crate) fn bar(ui: &mut Ui, frac: f32, width: f32, color: Color32, text: String) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 14.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
        let mut fill = rect;
        fill.set_width((width * frac.clamp(0.0, 1.0)).max(1.0));
        p.rect_filled(fill, 2.0, color);
        ui.label(RichText::new(text).small());
    });
}

pub(crate) fn stat(ui: &mut Ui, label: &str, value: String, color: Option<Color32>) {
    ui.vertical(|ui| {
        ui.weak(label);
        let t = RichText::new(value).size(18.0).strong();
        ui.label(match color {
            Some(c) => t.color(c),
            None => t,
        });
    });
    ui.add_space(12.0);
}

// ---------- 總覽 ----------

fn overview(ui: &mut Ui, icons: &mut IconStore, st: &Statics, c: &CompRow) {
    let d = &c.detail;
    let items_of =
        |u: &str| d.unit_items.iter().find(|(id, _)| id == u).map(|(_, v)| v.as_slice()).unwrap_or(&[]);

    // 核心、彈性各一排；英雄多時自動換行（不用直向分隔線，避免它把下方內容擠出視窗）
    for (label, list) in [("核心", &d.core), ("彈性", &d.flex)] {
        if list.is_empty() {
            continue;
        }
        ui.weak(label);
        ui.horizontal_wrapped(|ui| {
            for (u, star, share) in list.iter() {
                unit_card(ui, icons, st, u, *star, items_of(u), *share);
            }
        });
        ui.add_space(4.0);
    }

    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        section(ui, &format!("數據（{} 場）", fmt_int(c.count as u64)));
        ui.horizontal(|ui| {
            stat(ui, "出場率", format!("{:.2}", d.play_rate), None);
            stat(ui, "平均名次", format!("{:.2}", c.place), Some(place_color(c.place)));
            stat(ui, "前四%", format!("{:.1}", c.top4), None);
            stat(ui, "吃雞%", format!("{:.1}", c.win), None);
            stat(ui, "LP", format!("{:+.1}", c.lp_delta), Some(delta_color(c.lp_delta)));
        });

        section(ui, "裝備推薦");
        for (tier, chunk) in d.item_tiers.chunks(5).enumerate().take(3) {
            let (label, color) = match tier {
                0 => ("S", Color32::from_rgb(240, 190, 60)),
                1 => ("A", Color32::from_rgb(220, 80, 170)),
                _ => ("B", Color32::from_rgb(90, 170, 240)),
            };
            ui.horizontal(|ui| {
                ui.add_sized([22.0, 40.0], egui::Label::new(RichText::new(label).size(22.0).strong().color(color)));
                for (item, users, delta) in chunk {
                    ui.vertical(|ui| {
                        ui.set_width(44.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        icon(ui, icons, st.icon(item), 34.0, None)
                            .on_hover_text(format!("{}（名次差 {:+.2}）", st.name(item), delta));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            for u in users.iter().take(2) {
                                unit_icon(ui, icons, st, u, 16.0);
                            }
                        });
                    });
                }
            });
            ui.add_space(4.0);
        }

        let ui = &mut cols[1];
        section(ui, "等級分布");
        for (label, frac, place) in &d.levels {
            ui.horizontal(|ui| {
                ui.add_sized([30.0, 30.0], egui::Label::new(RichText::new(label).strong()));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    bar(ui, *frac, 180.0, BAR_PURPLE, format!("{:.1}%", frac * 100.0));
                    bar(ui, (8.0 - place) / 7.0, 180.0, BAR_TEAL, format!("平均 {place:.2} 名"));
                });
            });
        }

        section(ui, "名次分布");
        let max = d.placements.iter().cloned().fold(0.0f32, f32::max).max(0.001);
        for (i, frac) in d.placements.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add_sized([30.0, 14.0], egui::Label::new(RichText::new(format!("{}", i + 1)).small()));
                bar(ui, frac / max, 180.0, if i < 4 { BAR_TEAL } else { BAR_RED }, format!("{:.1}%", frac * 100.0));
            });
        }

        section(ui, "主力英雄");
        let maxw = d.carries.iter().map(|c| c.1).fold(0.0f32, f32::max).max(0.001);
        for (u, w) in &d.carries {
            ui.horizontal(|ui| {
                unit_icon(ui, icons, st, u, 28.0);
                bar(ui, w / maxw, 180.0, BAR_PURPLE, format!("{w:.2}"));
            });
        }
    });

    section(ui, "勝利條件（英雄星級 → 平均名次）");
    ui.horizontal_wrapped(|ui| {
        for (u, star, share, place) in &d.win_cons {
            ui.vertical(|ui| {
                ui.set_width(56.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.vertical_centered(|ui| {
                    ui.label(stars(*star));
                    icon(ui, icons, st.icon(u), 40.0, Some(cost_color(st.cost(u)))).on_hover_text(st.name(u));
                    ui.label(RichText::new(format!("{:.1}%", share * 100.0)).small());
                    ui.label(RichText::new(format!("{place:.2}")).small().color(place_color(*place)));
                });
            });
        }
    });
}

// ---------- 變化陣容 ----------

fn variant_grid(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, list: &[Variant]) {
    egui::Grid::new(id).striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
        for h in ["陣容", "場數", "平均名次", "前四%", "吃雞%"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for v in list {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let mut units: Vec<&String> = v.units.iter().collect();
                units.sort_by_key(|u| std::cmp::Reverse(st.cost(u)));
                for u in units {
                    unit_icon(ui, icons, st, u, 30.0);
                }
                if !v.spat.is_empty() {
                    ui.add_space(6.0);
                    for s in &v.spat {
                        item_icon(ui, icons, st, s, 24.0);
                    }
                }
            });
            ui.label(fmt_int(v.count as u64));
            ui.label(RichText::new(format!("{:.2}", v.place)).color(place_color(v.place)).strong());
            ui.label(format!("{:.1}%", v.top4));
            ui.label(format!("{:.1}%", v.win));
            ui.end_row();
        }
    });
}

fn variants(ui: &mut Ui, icons: &mut IconStore, st: &Statics, d: &CompDetail) {
    section(ui, "常見變化");
    variant_grid(ui, icons, st, "variants", &d.variants);
    if !d.lvl9.is_empty() {
        section(ui, "9 級陣容");
        variant_grid(ui, icons, st, "lvl9", &d.lvl9);
    }
}

// ---------- 羈絆 / 玩家 ----------

fn traits(ui: &mut Ui, icons: &mut IconStore, st: &Statics, d: &CompDetail) {
    egui::Grid::new("traits").striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
        for h in ["羈絆", "人數", "出現率", "平均名次"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for (t, tier, share, place) in &d.traits {
            ui.horizontal(|ui| {
                icon(ui, icons, st.icon(t), 24.0, Some(tier_color(*tier)));
                ui.label(st.name(t));
            });
            match st.trait_units(t, *tier) {
                Some(n) => ui.label(n.to_string()),
                None => ui.weak(format!("階段 {tier}")),
            };
            bar(ui, *share, 140.0, BAR_PURPLE, format!("{:.1}%", share * 100.0));
            ui.label(RichText::new(format!("{place:.2}")).color(place_color(*place)));
            ui.end_row();
        }
    });
}

fn players(ui: &mut Ui, d: &CompDetail) {
    ui.weak("使用這套陣容表現最好的玩家");
    ui.add_space(6.0);
    egui::Grid::new("players").striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
        for h in ["#", "伺服器", "玩家"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for (i, (region, name)) in d.top_players.iter().enumerate() {
            ui.weak((i + 1).to_string());
            ui.label(region.to_uppercase());
            ui.label(name);
            ui.end_row();
        }
    });
}
