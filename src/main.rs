#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod models;
mod storage;

use std::{fs, sync::Arc};

use eframe::{NativeOptions, egui};

fn main() -> eframe::Result<()> {
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1480.0, 920.0])
            .with_min_inner_size([1180.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "医疗产品进销存",
        native_options,
        Box::new(|cc| {
            install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(app::MedicalInventoryApp::new(cc)))
        }),
    )
}

fn install_cjk_font(ctx: &egui::Context) {
    let font_candidates = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];

    let Some(bytes) = font_candidates
        .iter()
        .find_map(|font_path| fs::read(font_path).ok())
    else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk_font".into(),
        Arc::new(egui::FontData::from_owned(bytes)),
    );

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk_font".into());
    }

    ctx.set_fonts(fonts);
}
