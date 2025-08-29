#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    // The Tauri app will handle logging configuration in lib.rs
    app_lib::run();
}
