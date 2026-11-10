#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sketch2asset_lib::matting::preload_startup_runtime_dlls();
    sketch2asset_lib::run()
}
