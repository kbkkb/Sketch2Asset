use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::AppResult;
use crate::library::db::LibraryDb;
use crate::matting::MattingEngine;
use crate::providers::comfyui::ComfyWorkflowConfig;
use crate::secrets::SecretStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub api_base: String,
    pub api_mode: String, // "images" | "chat" | "comfyui"
    pub api_model: String,
    /// Enabled models shown in the bottom Command Dock of Asset Forge (right shuttle pane)
    pub models: Vec<String>,
    /// Full pool of models fetched from remote `/v1/models` (left shuttle pane)
    #[serde(default)]
    pub fetched_models: Vec<String>,
    #[serde(default)]
    pub has_key: bool,
    /// Local ComfyUI workflow & slot binding configuration
    #[serde(default)]
    pub comfy_config: Option<ComfyWorkflowConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub version: u32,
    pub theme: String,    // "dark" | "light"
    pub language: String, // "zh-CN" | "en"
    pub active_profile_id: String,
    pub profiles: Vec<ProviderProfile>,
    pub active_project: String,
    pub default_archetype: String,
    pub default_style: String,
    pub default_background: String,
    pub default_size: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        let default_id = "prof_default".to_string();
        Self {
            version: 3,
            theme: "dark".to_string(),
            language: "zh-CN".to_string(),
            active_profile_id: default_id.clone(),
            profiles: vec![ProviderProfile {
                id: default_id,
                name: "Default Provider".to_string(),
                api_base: "https://api.openai.com/v1".to_string(),
                api_mode: "images".to_string(),
                api_model: String::new(),
                models: Vec::new(),
                fetched_models: Vec::new(),
                has_key: false,
                comfy_config: None,
            }],
            active_project: String::new(),
            default_archetype: "item_icon".to_string(),
            default_style: "default".to_string(),
            default_background: "white".to_string(),
            default_size: "1024x1024".to_string(),
        }
    }
}

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub library_root: PathBuf,
    pub settings_path: PathBuf,
    pub settings: Mutex<AppSettings>,
    pub secrets: SecretStore,
    pub db: LibraryDb,
    pub matting: std::sync::Arc<MattingEngine>,
    pub http: reqwest::Client,
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            let _ = fs::copy(&from, &to);
        }
    }
    Ok(())
}

impl AppState {
    pub fn initialize(app_data_dir: PathBuf, workspace_dir: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(&app_data_dir)?;

        // 1. All user-generated gallery assets & exports live safely in C: drive user profile
        let library_root = app_data_dir.join("outputs");
        fs::create_dir_all(&library_root)?;

        // If legacy outputs existed in workspace_dir (e.g. D:\Sketch2Asset\outputs), safely migrate to C: drive
        let legacy_outputs = workspace_dir.join("outputs");
        if legacy_outputs.exists() && legacy_outputs != library_root {
            if let Ok(entries) = fs::read_dir(&legacy_outputs) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') {
                        continue;
                    }
                    let dest = library_root.join(&name);
                    if !dest.exists() {
                        if path.is_dir() {
                            let _ = copy_dir_recursive(&path, &dest);
                        } else {
                            let _ = fs::copy(&path, &dest);
                        }
                    }
                }
            }
            // Safely clean up legacy outputs folder from the code workspace
            let _ = fs::remove_dir_all(&legacy_outputs);
        }

        // 2. Database lives in C: drive AppData
        let db_dir = app_data_dir.join("data");
        fs::create_dir_all(&db_dir)?;
        let target_db = db_dir.join("library.db");
        let legacy_db = app_data_dir.join("outputs").join(".sketch2asset").join("library.db");
        if legacy_db.exists() && !target_db.exists() {
            let _ = fs::copy(&legacy_db, &target_db);
            let _ = fs::remove_dir_all(app_data_dir.join("outputs").join(".sketch2asset"));
        }

        let secrets = SecretStore::new(&app_data_dir);
        let settings_path = app_data_dir.join("settings.json");

        let mut settings = if settings_path.exists() {
            fs::read_to_string(&settings_path)
                .ok()
                .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
                .unwrap_or_default()
        } else {
            // Migrate from v1 config.json if present
            let legacy_cfg = workspace_dir.join("config.json");
            if legacy_cfg.exists() {
                Self::migrate_from_v1(&legacy_cfg, &secrets).unwrap_or_default()
            } else {
                AppSettings::default()
            }
        };

        // 3. Move legacy config.json and matting_config.json containing plaintext keys out of workspace into C: drive backup
        let legacy_backup_dir = app_data_dir.join("legacy_backup");
        let _ = fs::create_dir_all(&legacy_backup_dir);
        let legacy_cfg = workspace_dir.join("config.json");
        if legacy_cfg.exists() {
            let backup_target = legacy_backup_dir.join("config.json");
            if !backup_target.exists() {
                let _ = fs::copy(&legacy_cfg, &backup_target);
            }
            let _ = fs::remove_file(&legacy_cfg);
        }
        let legacy_matting_cfg = workspace_dir.join("matting_config.json");
        if legacy_matting_cfg.exists() {
            let backup_target = legacy_backup_dir.join("matting_config.json");
            if !backup_target.exists() {
                let _ = fs::copy(&legacy_matting_cfg, &backup_target);
            }
            let _ = fs::remove_file(&legacy_matting_cfg);
        }

        if settings.profiles.is_empty() {
            settings = AppSettings::default();
        }

        // Migrate v2 -> v3: strip any hardcoded preset models that were injected before the user fetched real models
        if settings.version < 3 {
            let legacy_defaults = [
                "gpt-image-1",
                "gpt-image-1-mini",
                "dall-e-3",
                "gemini-2.5-flash-image",
                "doubao-seedream-4-0-250828",
                "flux-kontext-pro",
            ];
            for prof in &mut settings.profiles {
                let all_fetched_are_legacy = !prof.fetched_models.is_empty()
                    && prof.fetched_models.len() <= 6
                    && prof
                        .fetched_models
                        .iter()
                        .all(|m| legacy_defaults.contains(&m.as_str()));
                if all_fetched_are_legacy {
                    prof.fetched_models.clear();
                }
                let fetched_set: std::collections::HashSet<&str> =
                    prof.fetched_models.iter().map(|s| s.as_str()).collect();
                prof.models.retain(|m| {
                    if legacy_defaults.contains(&m.as_str()) {
                        // Only keep if it actually exists in a real fetched pool (>6 models)
                        prof.fetched_models.len() > 6 && fetched_set.contains(m.as_str())
                    } else {
                        true
                    }
                });
                if !prof.models.contains(&prof.api_model) {
                    prof.api_model = prof.models.first().cloned().unwrap_or_default();
                }
            }
            settings.version = 3;
        }

        for prof in &mut settings.profiles {
            prof.has_key = secrets.has_key(&prof.id) || prof.api_mode == "comfyui";
        }
        if !settings
            .profiles
            .iter()
            .any(|p| p.id == settings.active_profile_id)
        {
            settings.active_profile_id = settings.profiles[0].id.clone();
        }

        let db = LibraryDb::open(&db_dir)?;
        db.sync_existing_outputs(&library_root)?;

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .connect_timeout(std::time::Duration::from_secs(20))
            .build()?;

        let matting = std::sync::Arc::new(MattingEngine::new(workspace_dir.clone(), &app_data_dir));

        let state = Self {
            app_data_dir,
            library_root,
            settings_path,
            settings: Mutex::new(settings),
            secrets,
            db,
            matting,
            http,
        };
        state.persist_settings()?;
        Ok(state)
    }

    fn migrate_from_v1(legacy_path: &Path, secrets: &SecretStore) -> Option<AppSettings> {
        let raw = fs::read_to_string(legacy_path).ok()?;
        let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let arr = val.get("profiles")?.as_array()?;
        let mut profiles = Vec::new();

        for (idx, item) in arr.iter().enumerate() {
            let id = format!("prof_v1_{}", idx + 1);
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Migrated Profile")
                .to_string();
            let api_base = item
                .get("apiBase")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let api_key = item
                .get("apiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let api_mode = item
                .get("apiMode")
                .and_then(|v| v.as_str())
                .unwrap_or("images")
                .to_string();
            let api_model = item
                .get("apiModel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let models: Vec<String> = item
                .get("models")
                .and_then(|v| v.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|m| m.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            if !api_key.is_empty() {
                secrets.set_key(&id, api_key);
            }
            profiles.push(ProviderProfile {
                id: id.clone(),
                name,
                api_base,
                api_mode: api_mode.clone(),
                api_model: api_model.clone(),
                models: if models.len() > 8 {
                    vec![api_model]
                } else {
                    models.clone()
                },
                fetched_models: models,
                has_key: !api_key.is_empty() || api_mode == "comfyui",
                comfy_config: None,
            });
        }

        if profiles.is_empty() {
            return None;
        }
        let active_idx = val.get("active").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let active_profile_id = profiles
            .get(active_idx)
            .unwrap_or(&profiles[0])
            .id
            .clone();

        Some(AppSettings {
            active_profile_id,
            profiles,
            active_project: val
                .get("projSel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            ..AppSettings::default()
        })
    }

    pub fn reload_settings_from_disk(&self) {
        if self.settings_path.exists() {
            if let Ok(raw) = fs::read_to_string(&self.settings_path) {
                if let Ok(mut disk_settings) = serde_json::from_str::<AppSettings>(&raw) {
                    if !disk_settings.profiles.is_empty() {
                        for prof in &mut disk_settings.profiles {
                            prof.has_key =
                                self.secrets.has_key(&prof.id) || prof.api_mode == "comfyui";
                        }
                        if let Ok(mut guard) = self.settings.lock() {
                            *guard = disk_settings;
                        }
                    }
                }
            }
        }
    }

    pub fn persist_settings(&self) -> AppResult<()> {
        let mut guard = self.settings.lock().unwrap();
        for prof in &mut guard.profiles {
            prof.has_key = self.secrets.has_key(&prof.id) || prof.api_mode == "comfyui";
        }
        let json = serde_json::to_string_pretty(&*guard).unwrap_or_default();
        let tmp = self.settings_path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.settings_path)?;
        Ok(())
    }
}
