//! Pigeon, a desktop API client whose collections sync through git.

// A finished Windows build opens only the app window, with no black console behind it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod cli;
mod crash;
mod ui;

use cli::Command;
use eframe::egui;
use pigeon_core::{APP_NAME, settings};
use std::sync::Arc;

const ICON: &[u8] = include_bytes!("../../../assets/icon/pigeon-256.png");

fn window_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(ICON).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData { rgba: image.into_raw(), width, height })
}

fn main() -> eframe::Result<()> {
    let screenshot = match cli::parse(std::env::args().skip(1)) {
        Command::Run { screenshot } => screenshot,
        Command::Print(text) => {
            println!("{text}");
            return Ok(());
        }
    };
    crash::install();
    if screenshot.is_none() {
        settings::adopt_previous_data();
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_app_id("pigeon")
        .with_inner_size([1360.0, 840.0])
        .with_min_inner_size([900.0, 560.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(APP_NAME, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot)))))
}
