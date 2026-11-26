#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cache;
mod data;
mod detail;
mod entity;
mod model;
mod net;
mod search;
mod ui;

use eframe::egui;
use std::time::Instant;

fn main() -> eframe::Result {
    let t0 = Instant::now();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TFT 統計")
            .with_inner_size([1600.0, 980.0])
            .with_min_inner_size([1000.0, 600.0]),
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "TFT 統計",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cc, t0)))
        }),
    )
}

/// 載入系統的微軟正黑體，讓中文能顯示。egui 只在用到某個字時才把字形畫進圖集。
/// 越南文、韓文、日文等玩家名稱用的備用字型在背景載入，不拖慢第一個畫面。
fn install_fonts(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let mut fonts = egui::FontDefinitions::default();
    let cjk = [("msjh", "msjh.ttc"), ("msyh", "msyh.ttc")];
    // 找到第一個存在的中文字型就停
    let _ = cjk.iter().any(|f| add_font(&mut fonts, &windir, *f));
    ctx.set_fonts(fonts.clone());

    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let extras = [("segoeui", "segoeui.ttf"), ("malgun", "malgun.ttf"), ("yugoth", "YuGothM.ttc")];
        let mut added = false;
        for f in extras {
            added |= add_font(&mut fonts, &windir, f);
        }
        if added {
            ctx.set_fonts(fonts);
            ctx.request_repaint();
        }
    });
}

/// 把字型加到預設英文字型後面：英數字用原字型，缺字時依序退回。
fn add_font(fonts: &mut egui::FontDefinitions, windir: &str, (name, file): (&str, &str)) -> bool {
    let Ok(bytes) = std::fs::read(format!(r"{windir}\Fonts\{file}")) else { return false };
    fonts.font_data.insert(name.to_owned(), egui::FontData::from_owned(bytes).into());
    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(fam).or_default().push(name.to_owned());
    }
    true
}
