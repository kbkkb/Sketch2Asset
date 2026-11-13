pub mod commands;
pub mod error;
pub mod library;
pub mod matting;
pub mod mcp;
pub mod postprocess;
pub mod providers;
pub mod secrets;
pub mod state;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedWindowBounds {
    width: u32,
    height: u32,
    x: Option<i32>,
    y: Option<i32>,
    maximized: bool,
}

fn load_window_bounds(path: &Path) -> Option<SavedWindowBounds> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_window_bounds(window: &tauri::Window, path: &Path) {
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let maximized = window.is_maximized().unwrap_or(false);
    let prev = load_window_bounds(path);

    let bounds = if maximized {
        SavedWindowBounds {
            width: prev.as_ref().map(|p| p.width).unwrap_or(1380),
            height: prev.as_ref().map(|p| p.height).unwrap_or(880),
            x: prev.as_ref().and_then(|p| p.x),
            y: prev.as_ref().and_then(|p| p.y),
            maximized: true,
        }
    } else {
        let size = match window.outer_size() {
            Ok(s) if s.width >= 800 && s.height >= 500 => s,
            _ => return,
        };
        let pos = window.outer_position().ok().filter(|p| p.x > -10000 && p.y > -10000);
        SavedWindowBounds {
            width: size.width,
            height: size.height,
            x: pos.map(|p| p.x),
            y: pos.map(|p| p.y),
            maximized: false,
        }
    };

    if let Ok(json) = serde_json::to_string_pretty(&bounds) {
        let _ = fs::write(path, json);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    matting::preload_startup_runtime_dlls();
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("com.sketch2asset.app"));

            let mut workspace_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

            if workspace_dir
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.eq_ignore_ascii_case("bin"))
                .unwrap_or(false)
            {
                if let Some(parent) = workspace_dir.parent() {
                    workspace_dir = parent.to_path_buf();
                }
            }

            let bounds_path = app_data_dir.join("window_bounds.json");
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_min_size(Some(tauri::LogicalSize::new(1180.0, 740.0)));
                if let Some(b) = load_window_bounds(&bounds_path) {
                    let w = b.width.max(1180);
                    let h = b.height.max(740);
                    let _ = win.set_size(tauri::PhysicalSize::new(w, h));
                    if let (Some(x), Some(y)) = (b.x, b.y) {
                        if x > -10000 && y > -10000 {
                            let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
                        }
                    }
                    if b.maximized {
                        let _ = win.maximize();
                    }
                }
            }

            let app_state = state::AppState::initialize(app_data_dir, workspace_dir)
                .expect("Failed to initialize Sketch2Asset AppState");

            app.manage(app_state);
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if matches!(
                event,
                tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::CloseRequested { .. }
            ) {
                let state = window.state::<state::AppState>();
                let bounds_path = state.app_data_dir.join("window_bounds.json");
                save_window_bounds(window, &bounds_path);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::set_api_key,
            commands::get_api_key,
            commands::get_api_key_masked,
            commands::fetch_provider_models,
            commands::analyze_comfyui_workflow,
            commands::generate_assets,
            commands::list_projects,
            commands::upsert_project,
            commands::rename_project,
            commands::list_assets,
            commands::delete_asset,
            commands::update_asset_archetype,
            commands::reveal_in_explorer,
            commands::export_config_bundle,
            commands::import_config_bundle,
            commands::list_matting_models,
            commands::download_matting_model,
            commands::delete_matting_model,
            commands::open_models_folder,
            commands::run_ai_matting,
            commands::read_image_data_url,
            commands::save_studio_asset,
            commands::export_studio_png,
            commands::run_studio_region_edit,
            commands::generate_character_asset,
            commands::check_app_update,
            commands::get_agent_hub_status,
            commands::install_agent_integration,
            commands::uninstall_agent_integration,
            commands::export_agent_skill_bundle,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sketch2Asset application");
}
