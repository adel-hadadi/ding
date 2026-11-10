//! A small window for Ding. egui draws it the same way on Ubuntu, macOS, and Windows.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod gui;

fn main() -> eframe::Result {
    gui::run()
}
