use std::sync::OnceLock;

use eframe::egui;
use egui_extras::install_image_loaders;

use crate::app::App;

mod app;
mod async_load;
mod settings;
mod steam;
mod tabs;
mod ui;
mod whitelist;

static CTX: OnceLock<egui::Context> = OnceLock::new();

pub fn ctx() -> &'static egui::Context {
    CTX.get().expect("Context not initialized")
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 400.0])
            .with_min_inner_size([600.0, 400.0]),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "gmod-wstool",
        options,
        Box::new(|cc| {
            CTX.set(cc.egui_ctx.clone()).ok();
            install_image_loaders(&cc.egui_ctx);
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(cc)))
        }),
    )
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "noto".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/NotoSans-Regular.ttf")).into(),
    );

    fonts.font_data.insert(
        "unifont".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/unifont-17.0.03.otf")).into(),
    );

    fonts.families.insert(
        egui::FontFamily::Proportional,
        vec!["noto".to_owned(), "unifont".to_owned()],
    );

    fonts.families.insert(
        egui::FontFamily::Monospace,
        vec!["noto".to_owned(), "unifont".to_owned()],
    );

    ctx.set_fonts(fonts);
}
