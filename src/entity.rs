//! 英雄、裝備的詳細資料視窗。資料點開時才抓（stats2/unit、stats2/item），並寫入快取。

use crate::app::{App, IconStore};
use crate::detail::{bar, section, stars, stat, BAR_RED, BAR_TEAL};
use crate::model::*;
use crate::ui::{cost_color, delta_color, fmt_int, icon, item_icon, place_color, tier_color, unit_icon};
use eframe::egui::{self, Color32, RichText, Ui};

#[derive(Clone, Copy, PartialEq)]
enum RelKind {
    Unit,
    Trait,
    Item,
}

// ---------- 英雄 ----------

pub fn unit_window(app: &mut App, ctx: &egui::Context) {
    let Some(id) = app.ui.selected_unit.clone() else { return };
    let key = EntityKey { is_item: false, filter: app.filter, id: id.clone() };
    app.ensure_entity(&key);
    let st = &app.statics;
    let icons = &mut app.icons;
    let tab = &mut app.ui.unit_tab;
    let d = app.entities.get(&key);
    let status = load_status(app.ent_pending.contains(&key), app.ent_errors.get(&key));
    let mut open = true;

    window(ctx, "unit_detail", st.name(&id), &mut open, |ui| {
        ui.horizontal(|ui| {
            icon(ui, icons, st.icon(&id), 64.0, Some(cost_color(st.cost(&id))));
            ui.vertical(|ui| {
                ui.label(RichText::new(st.name(&id)).size(22.0).strong());
                let c = st.cost(&id);
                ui.label(RichText::new(format!("{c} 費")).color(cost_color(c)));
                if let Some(e) = st.entry(&id) {
                    ui.weak(e.traits.join(" · "));
                }
            });
        });
        let Some(d) = d else { return status_view(ui, &status) };
        tabs(ui, tab, &["總覽", "出裝", "搭配", "伺服器"], &status);
        match *tab {
            1 => {
                section(ui, "三件組合（依場數）");
                builds_grid(ui, icons, st, "b3", &d.builds3, d.base.count, 15);
                section(ui, "兩件組合");
                builds_grid(ui, icons, st, "b2", &d.builds2, d.base.count, 12);
                section(ui, "單件裝備");
                builds_grid(ui, icons, st, "b1", &d.builds1, d.base.count, 20);
            }
            2 => {
                section(ui, "最佳搭配羈絆（名次差越負越好）");
                rel_grid(ui, icons, st, "rt", &d.traits, RelKind::Trait, d.base.count, 15);
                section(ui, "最佳隊友");
                rel_grid(ui, icons, st, "ru", &d.units, RelKind::Unit, d.base.count, 15);
            }
            3 => regions(ui, &d.regions),
            _ => {
                base_stats(ui, d);
                ui.columns(2, |cols| {
                    let ui = &mut cols[0];
                    section(ui, "星級");
                    perf_grid(ui, "stars", "星級", &d.stars, d.base.count, |ui, n| {
                        ui.label(stars(n).size(16.0));
                    });
                    section(ui, "裝備數量");
                    perf_grid(ui, "icount", "件數", &d.item_counts, d.base.count, |ui, n| {
                        ui.label(format!("{n} 件"));
                    });
                    placements(&mut cols[1], &d.placements);
                });
            }
        }
    });

    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.ui.selected_unit = None;
    }
}

// ---------- 裝備 ----------

pub fn item_window(app: &mut App, ctx: &egui::Context) {
    let Some(id) = app.ui.selected_item.clone() else { return };
    let key = EntityKey { is_item: true, filter: app.filter, id: id.clone() };
    app.ensure_entity(&key);
    let st = &app.statics;
    let icons = &mut app.icons;
    let tab = &mut app.ui.item_tab;
    let d = app.entities.get(&key);
    let status = load_status(app.ent_pending.contains(&key), app.ent_errors.get(&key));
    let mut open = true;

    window(ctx, "item_detail", st.name(&id), &mut open, |ui| {
        ui.horizontal(|ui| {
            icon(ui, icons, st.icon(&id), 64.0, None);
            ui.vertical(|ui| {
                ui.label(RichText::new(st.name(&id)).size(22.0).strong());
                if let Some(e) = st.entry(&id).filter(|e| !e.parts.is_empty()) {
                    ui.horizontal(|ui| {
                        ui.weak("合成：");
                        for (i, p) in e.parts.iter().enumerate() {
                            if i > 0 {
                                ui.weak("+");
                            }
                            item_icon(ui, icons, st, p, 26.0);
                        }
                    });
                }
            });
        });
        let Some(d) = d else { return status_view(ui, &status) };
        tabs(ui, tab, &["總覽", "適合英雄", "搭配", "伺服器"], &status);
        match *tab {
            1 => {
                section(ui, "持有這件裝備的英雄（依場數）");
                rel_grid_by_count(ui, icons, st, "iu", &d.units, d.base.count, 30);
            }
            2 => {
                section(ui, "最佳搭配羈絆（名次差越負越好）");
                rel_grid(ui, icons, st, "it", &d.traits, RelKind::Trait, d.base.count, 15);
                section(ui, "常一起出現的裝備");
                rel_grid(ui, icons, st, "ii", &d.items, RelKind::Item, d.base.count, 15);
            }
            3 => regions(ui, &d.regions),
            _ => {
                base_stats(ui, d);
                ui.columns(2, |cols| {
                    let ui = &mut cols[0];
                    section(ui, "最常持有的英雄（佔比 · 平均名次）");
                    let top = d.units.first().map(|r| r.perf.count).unwrap_or(1).max(1) as f32;
                    for r in d.units.iter().take(8) {
                        ui.horizontal(|ui| {
                            unit_icon(ui, icons, st, &r.id, 28.0);
                            let frac = r.perf.count as f32 / d.base.count.max(1) as f32;
                            let len = r.perf.count as f32 / top;
                            bar(ui, len, 150.0, crate::detail::BAR_PURPLE, format!("{:.1}% · {:.2}", frac * 100.0, r.perf.place));
                        });
                    }
                    placements(&mut cols[1], &d.placements);
                });
            }
        }
    });

    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.ui.selected_item = None;
    }
}

// ---------- 共用 ----------

enum Status {
    Ready,
    Loading,
    Error(String),
}

fn load_status(pending: bool, err: Option<&String>) -> Status {
    match (pending, err) {
        (true, _) => Status::Loading,
        (false, Some(e)) => Status::Error(e.clone()),
        _ => Status::Ready,
    }
}

fn status_view(ui: &mut Ui, s: &Status) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| match s {
        Status::Error(e) => {
            ui.colored_label(Color32::from_rgb(230, 90, 90), e);
        }
        _ => {
            ui.spinner();
        }
    });
}

fn window(ctx: &egui::Context, id: &str, title: &str, open: &mut bool, body: impl FnOnce(&mut Ui)) {
    egui::Window::new(RichText::new(title).size(18.0).strong())
        .id(egui::Id::new(id))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .default_size([700.0, 640.0])
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center() + egui::vec2(30.0, 20.0))
        .vscroll(true)
        .show(ctx, body);
}

fn tabs(ui: &mut Ui, tab: &mut u8, labels: &[&str], status: &Status) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for (i, l) in labels.iter().enumerate() {
            ui.selectable_value(tab, i as u8, RichText::new(*l).size(15.0));
        }
        if matches!(status, Status::Loading) {
            ui.spinner();
            ui.weak("更新中…");
        }
    });
    ui.separator();
}

fn base_stats(ui: &mut Ui, d: &EntityDetail) {
    section(ui, &format!("數據（{} 場）", fmt_int(d.base.count as u64)));
    ui.horizontal(|ui| {
        stat(ui, "出場率", format!("{:.2}", d.rate), None);
        stat(ui, "平均名次", format!("{:.2}", d.base.place), Some(place_color(d.base.place)));
        stat(ui, "前四%", format!("{:.1}", d.base.top4), None);
        stat(ui, "吃雞%", format!("{:.1}", d.base.won), None);
    });
}

fn placements(ui: &mut Ui, list: &[f32]) {
    section(ui, "名次分布");
    let max = list.iter().cloned().fold(0.0f32, f32::max).max(0.001);
    for (i, frac) in list.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_sized([24.0, 14.0], egui::Label::new(RichText::new(format!("{}", i + 1)).small()));
            bar(ui, frac / max, 170.0, if i < 4 { BAR_TEAL } else { BAR_RED }, format!("{:.1}%", frac * 100.0));
        });
    }
}

fn perf_grid(ui: &mut Ui, id: &str, head: &str, list: &[(u8, Perf)], total: u32, label: impl Fn(&mut Ui, u8)) {
    egui::Grid::new(id).striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
        for h in [head, "佔比", "平均名次", "前四%", "吃雞%"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for (n, p) in list {
            label(ui, *n);
            ui.label(format!("{:.1}%", p.count as f32 * 100.0 / total.max(1) as f32));
            ui.label(RichText::new(format!("{:.2}", p.place)).color(place_color(p.place)).strong());
            ui.label(format!("{:.1}%", p.top4));
            ui.label(format!("{:.1}%", p.won));
            ui.end_row();
        }
    });
}

fn delta_label(ui: &mut Ui, d: f32) {
    // 名次差：負的代表名次更好
    ui.label(RichText::new(format!("{d:+.2}")).color(delta_color(-d)));
}

fn builds_grid(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, list: &[Build], total: u32, limit: usize) {
    egui::Grid::new(id).striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
        for h in ["裝備", "使用率", "場數", "平均名次", "前四%", "名次差"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for b in list.iter().take(limit) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for it in &b.items {
                    item_icon(ui, icons, st, it, 30.0);
                }
            });
            ui.label(format!("{:.1}%", b.perf.count as f32 * 100.0 / total.max(1) as f32));
            ui.label(fmt_int(b.perf.count as u64));
            ui.label(RichText::new(format!("{:.2}", b.perf.place)).color(place_color(b.perf.place)).strong());
            ui.label(format!("{:.1}%", b.perf.top4));
            delta_label(ui, b.delta);
            ui.end_row();
        }
    });
}

fn rel_cell(ui: &mut Ui, icons: &mut IconStore, st: &Statics, r: &Rel, kind: RelKind) {
    ui.horizontal(|ui| match kind {
        RelKind::Unit => {
            unit_icon(ui, icons, st, &r.id, 28.0);
            ui.label(st.name(&r.id));
        }
        RelKind::Item => {
            item_icon(ui, icons, st, &r.id, 28.0);
            ui.label(st.name(&r.id));
        }
        RelKind::Trait => {
            icon(ui, icons, st.icon(&r.id), 24.0, Some(tier_color(r.tier)));
            let n = st.trait_units(&r.id, r.tier).map(|n| format!(" {n}")).unwrap_or_default();
            ui.label(format!("{}{n}", st.name(&r.id)));
        }
    });
}

/// 依名次差排序（只看出現率 >= 3% 的，避免小樣本）
fn rel_grid(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, list: &[Rel], kind: RelKind, total: u32, limit: usize) {
    let min = (total as f32 * 0.03) as u32;
    let mut v: Vec<&Rel> = list.iter().filter(|r| r.perf.count >= min).collect();
    v.sort_by(|a, b| a.delta.total_cmp(&b.delta));
    rel_table(ui, icons, st, id, &v[..v.len().min(limit)], kind, total);
}

fn rel_grid_by_count(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, list: &[Rel], total: u32, limit: usize) {
    let v: Vec<&Rel> = list.iter().take(limit).collect();
    rel_table(ui, icons, st, id, &v, RelKind::Unit, total);
}

fn rel_table(ui: &mut Ui, icons: &mut IconStore, st: &Statics, id: &str, v: &[&Rel], kind: RelKind, total: u32) {
    egui::Grid::new(id).striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
        for h in ["", "出現率", "場數", "平均名次", "前四%", "名次差"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for r in v {
            rel_cell(ui, icons, st, r, kind);
            ui.label(format!("{:.1}%", r.perf.count as f32 * 100.0 / total.max(1) as f32));
            ui.label(fmt_int(r.perf.count as u64));
            ui.label(RichText::new(format!("{:.2}", r.perf.place)).color(place_color(r.perf.place)).strong());
            ui.label(format!("{:.1}%", r.perf.top4));
            delta_label(ui, r.delta);
            ui.end_row();
        }
    });
}

fn regions(ui: &mut Ui, list: &[(String, Perf)]) {
    section(ui, "各伺服器");
    egui::Grid::new("regions").striped(true).spacing([20.0, 6.0]).show(ui, |ui| {
        for h in ["伺服器", "場數", "平均名次", "前四%", "吃雞%"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for (r, p) in list {
            ui.label(r);
            ui.label(fmt_int(p.count as u64));
            ui.label(RichText::new(format!("{:.2}", p.place)).color(place_color(p.place)).strong());
            ui.label(format!("{:.1}%", p.top4));
            ui.label(format!("{:.1}%", p.won));
            ui.end_row();
        }
    });
}
