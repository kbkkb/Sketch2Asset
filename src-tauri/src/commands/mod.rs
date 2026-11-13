use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;

use crate::error::{AppError, AppErrorCode, AppResult};
use crate::library::db::{AssetItem, ProjectInfo};
use crate::matting::MattingModelInfo;
use crate::providers;
use crate::state::{AppSettings, AppState, ProviderProfile};

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.reload_settings_from_disk();
    let mut guard = state.settings.lock().unwrap();
    for prof in &mut guard.profiles {
        prof.has_key = state.secrets.has_key(&prof.id) || prof.api_mode == "comfyui";
    }
    Ok(guard.clone())
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, mut new_settings: AppSettings) -> AppResult<AppSettings> {
    if new_settings.profiles.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "At least one provider profile is required",
        ));
    }
    for prof in &mut new_settings.profiles {
        prof.has_key = state.secrets.has_key(&prof.id) || prof.api_mode == "comfyui";
    }
    {
        let mut guard = state.settings.lock().unwrap();
        *guard = new_settings;
    }
    state.persist_settings()?;
    get_settings(state)
}

#[tauri::command]
pub fn set_api_key(
    state: State<'_, AppState>,
    profile_id: String,
    api_key: String,
) -> AppResult<AppSettings> {
    state.secrets.set_key(&profile_id, &api_key);
    state.persist_settings()?;
    get_settings(state)
}

#[tauri::command]
pub fn get_api_key(state: State<'_, AppState>, profile_id: String) -> AppResult<String> {
    Ok(state.secrets.get_key(&profile_id).unwrap_or_default())
}

#[tauri::command]
pub fn get_api_key_masked(state: State<'_, AppState>, profile_id: String) -> AppResult<String> {
    if let Some(k) = state.secrets.get_key(&profile_id) {
        let len = k.chars().count();
        if len <= 8 {
            return Ok("••••••••".to_string());
        }
        let prefix: String = k.chars().take(5).collect();
        let suffix: String = k.chars().skip(len - 4).collect();
        Ok(format!("{}••••••{}", prefix, suffix))
    } else {
        Ok(String::new())
    }
}

#[tauri::command]
pub async fn fetch_provider_models(
    state: State<'_, AppState>,
    profile_id: String,
    api_base: String,
    api_key_override: Option<String>,
) -> AppResult<Vec<String>> {
    let api_mode = {
        let guard = state.settings.lock().unwrap();
        guard
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .map(|p| p.api_mode.clone())
            .unwrap_or_else(|| "images".to_string())
    };

    let models = if api_mode == "comfyui" {
        let opt_key = match api_key_override.filter(|s| !s.trim().is_empty()) {
            Some(k) => {
                state.secrets.set_key(&profile_id, &k);
                Some(k)
            }
            None => state.secrets.get_key(&profile_id),
        };
        providers::comfyui::fetch_comfy_models(&state.http, &api_base, opt_key.as_deref()).await?
    } else {
        let key = match api_key_override.filter(|s| !s.trim().is_empty()) {
            Some(k) => {
                state.secrets.set_key(&profile_id, &k);
                k
            }
            None => state.secrets.get_key(&profile_id).ok_or_else(|| {
                AppError::new(AppErrorCode::MissingApiKey, "API Key not configured for this profile")
            })?,
        };
        providers::fetch_models(&state.http, &api_base, &key).await?
    };

    {
        let mut guard = state.settings.lock().unwrap();
        if let Some(p) = guard.profiles.iter_mut().find(|p| p.id == profile_id) {
            p.api_base = api_base;
            p.fetched_models = models.clone();
            if p.models.is_empty() {
                if !p.api_model.trim().is_empty() {
                    p.models.push(p.api_model.clone());
                } else if api_mode == "comfyui" {
                    if let Some(first) = models.first() {
                        p.models.push(first.clone());
                        p.api_model = first.clone();
                    }
                }
            }
        }
    }
    state.persist_settings()?;
    Ok(models)
}

#[tauri::command]
pub async fn analyze_comfyui_workflow(
    state: State<'_, AppState>,
    api_base: String,
    raw_content: String,
) -> AppResult<providers::comfyui::ComfyWorkflowAnalysis> {
    providers::comfyui::analyze_comfy_workflow(&state.http, &api_base, &raw_content).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratePayload {
    pub profile_id: String,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub archetype: String,
    pub archetype_prompt: String,
    pub style_prompt: String,
    pub project_style_prompt: String,
    pub background_prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    pub size: String,
    pub project: String,
    pub count: usize,
    pub sketch_data_url: Option<String>,
    pub sketch_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResponse {
    pub assets: Vec<AssetItem>,
    pub elapsed_ms: u64,
}

#[tauri::command]
pub async fn generate_assets(
    state: State<'_, AppState>,
    payload: GeneratePayload,
) -> AppResult<GenerateResponse> {
    let start_ts = std::time::Instant::now();

    let (api_base, api_mode, model, comfy_cfg) = {
        let guard = state.settings.lock().unwrap();
        let prof = guard
            .profiles
            .iter()
            .find(|p| p.id == payload.profile_id)
            .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "Profile not found"))?;
        let m = if payload.model.trim().is_empty() {
            prof.api_model.clone()
        } else {
            payload.model.trim().to_string()
        };
        (
            prof.api_base.clone(),
            prof.api_mode.clone(),
            m,
            prof.comfy_config.clone(),
        )
    };

    let opt_key = state.secrets.get_key(&payload.profile_id);
    if api_mode != "comfyui" && opt_key.is_none() {
        return Err(AppError::new(
            AppErrorCode::MissingApiKey,
            "Please configure the API Key for the active profile first",
        ));
    }
    let api_key = opt_key.unwrap_or_default();

    // Resolve optional sketch input
    let sketch_bytes = resolve_sketch_input(&payload)?;

    // Assemble final structured game asset prompt
    let final_prompt = assemble_game_asset_prompt(&payload, sketch_bytes.is_some());

    let batch_count = payload.count.clamp(1, 16);
    let mut created_assets = Vec::new();

    for _ in 0..batch_count {
        let (img_bytes, via) = if api_mode == "comfyui" {
            providers::comfyui::generate_via_comfyui(
                &state.http,
                &api_base,
                if api_key.is_empty() { None } else { Some(&api_key) },
                &model,
                &final_prompt,
                &payload.negative_prompt,
                &payload.size,
                sketch_bytes.clone(),
                None,
                comfy_cfg.as_ref(),
            )
            .await?
        } else if api_mode == "chat" {
            providers::generate_via_chat(
                &state.http,
                &api_base,
                &api_key,
                &model,
                &final_prompt,
                sketch_bytes.clone(),
            )
            .await?
        } else {
            providers::generate_via_images(
                &state.http,
                &api_base,
                &api_key,
                &model,
                &final_prompt,
                &payload.size,
                sketch_bytes.clone(),
            )
            .await?
        };

        let asset = save_generated_image(
            &state,
            &img_bytes,
            &payload.project,
            &payload.archetype,
            &payload.prompt,
            &model,
            &via,
            Some(&payload.size),
        )?;
        created_assets.push(asset);
    }

    Ok(GenerateResponse {
        assets: created_assets,
        elapsed_ms: start_ts.elapsed().as_millis() as u64,
    })
}

pub(crate) fn assemble_game_asset_prompt(p: &GeneratePayload, has_sketch: bool) -> String {
    let mut parts = Vec::new();
    if has_sketch {
        parts.push("Based on the provided reference/sketch composition and silhouette, render a high-quality game asset:".to_string());
    }
    if !p.prompt.trim().is_empty() {
        parts.push(p.prompt.trim().to_string());
    }
    if !p.archetype_prompt.trim().is_empty() {
        parts.push(p.archetype_prompt.trim().to_string());
    }
    if !p.style_prompt.trim().is_empty() {
        parts.push(p.style_prompt.trim().to_string());
    }
    if !p.project_style_prompt.trim().is_empty() {
        parts.push(format!("Project visual style rules: {}", p.project_style_prompt.trim()));
    }
    if !p.background_prompt.trim().is_empty() {
        let bg = p.background_prompt.trim();
        let bg_upper = bg.to_ascii_uppercase();
        if bg_upper.starts_with("CRITICAL BACKGROUND") || bg_upper.starts_with("BACKGROUND RULE") {
            parts.push(bg.to_string());
        } else {
            parts.push(format!("CRITICAL BACKGROUND RULE: {}", bg));
        }
    }
    if let Some((w, h)) = providers::parse_size_dims(&p.size) {
        let ratio = w as f64 / h as f64;
        let orient = if (ratio - 16.0 / 9.0).abs() < 0.08 {
            "16:9 widescreen landscape"
        } else if (ratio - 9.0 / 16.0).abs() < 0.08 {
            "9:16 vertical portrait"
        } else if ratio > 1.15 {
            "3:2 horizontal landscape"
        } else if ratio < 0.87 {
            "2:3 vertical portrait"
        } else {
            "1:1 square"
        };
        parts.push(format!(
            "Target Canvas Aspect Ratio & Resolution: {} ({}x{}px).",
            orient, w, h
        ));
    }
    if !p.negative_prompt.trim().is_empty() {
        parts.push(format!("STRICT AVOID / NEGATIVE CONSTRAINTS: {}", p.negative_prompt.trim()));
    }
    parts.join("\n\n")
}

pub(crate) fn resolve_sketch_input(p: &GeneratePayload) -> AppResult<Option<(Vec<u8>, String)>> {
    if let Some(path_str) = p
        .sketch_path
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(AppError::new(
                AppErrorCode::NotFound,
                format!("Sketch/reference image not found: {}", path_str),
            ));
        }
        let bytes = fs::read(&path).map_err(|e| {
            AppError::new(
                AppErrorCode::NotFound,
                format!("Failed to read sketch/reference image '{}': {}", path_str, e),
            )
        })?;
        let mime = match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            _ => "image/png",
        };
        return Ok(Some((bytes, mime.to_string())));
    }
    if let Some(data_url) = p
        .sketch_data_url
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Some(idx) = data_url.find(";base64,") {
            let mime = data_url[..idx]
                .strip_prefix("data:")
                .unwrap_or("image/png")
                .to_string();
            let raw_b64 = &data_url[idx + 8..];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw_b64.trim())
                .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;
            return Ok(Some((bytes, mime)));
        } else {
            return Err(AppError::new(
                AppErrorCode::InvalidInput,
                "Invalid sketch_data_url format: expected 'data:<mime>;base64,...'",
            ));
        }
    }
    Ok(None)
}

pub(crate) fn fit_image_to_target_size(img: image::DynamicImage, target_w: u32, target_h: u32) -> image::DynamicImage {
    use image::{ GenericImageView, RgbaImage, imageops::FilterType };
    let sw = img.width();
    let sh = img.height();
    if sw == target_w && sh == target_h {
        return img;
    }
    let src_ratio = sw as f64 / sh.max(1) as f64;
    let dst_ratio = target_w as f64 / target_h.max(1) as f64;
    if (src_ratio - dst_ratio).abs() <= 0.04 {
        return img.resize_exact(target_w, target_h, FilterType::Lanczos3);
    }
    // Preserve subject aspect ratio without distortion; pad margins with corner background color
    let scale = (target_w as f64 / sw as f64).min(target_h as f64 / sh as f64);
    let nw = ((sw as f64 * scale).round() as u32).clamp(1, target_w);
    let nh = ((sh as f64 * scale).round() as u32).clamp(1, target_h);
    let resized = img.resize_exact(nw, nh, FilterType::Lanczos3).to_rgba8();

    let corner_px = img.get_pixel(0, 0);
    let mut canvas = RgbaImage::from_pixel(target_w, target_h, corner_px);
    let ox = ((target_w - nw) / 2) as i64;
    let oy = ((target_h - nh) / 2) as i64;
    image::imageops::overlay(&mut canvas, &resized, ox, oy);
    image::DynamicImage::ImageRgba8(canvas)
}

pub(crate) fn save_generated_image(
    state: &AppState,
    raw_bytes: &[u8],
    project: &str,
    archetype: &str,
    user_prompt: &str,
    model: &str,
    via: &str,
    requested_size: Option<&str>,
) -> AppResult<AssetItem> {
    let proj_clean = sanitize_project_name(project);
    let target_dir = if proj_clean.is_empty() {
        state.library_root.clone()
    } else {
        state.library_root.join(&proj_clean)
    };
    fs::create_dir_all(&target_dir)?;

    let now_dt = chrono::Local::now();
    let ts_str = now_dt.format("%Y%m%d_%H%M%S").to_string();
    let short_uid = &uuid::Uuid::new_v4().simple().to_string()[..6];
    let file_name = format!("{}_{}_gen.png", ts_str, short_uid);
    let file_path = target_dir.join(&file_name);

    // Decode, optionally fit to requested target WxH, and normalize to PNG
    let (width, height) = if let Ok(mut img) = image::load_from_memory(raw_bytes) {
        if let Some(sz) = requested_size {
            if let Some((tw, th)) = providers::parse_size_dims(sz) {
                if img.width() != tw || img.height() != th {
                    img = fit_image_to_target_size(img, tw, th);
                }
            }
        }
        let w = img.width();
        let h = img.height();
        img.save_with_format(&file_path, image::ImageFormat::Png)
            .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
        (w, h)
    } else {
        fs::write(&file_path, raw_bytes)?;
        image::image_dimensions(&file_path).unwrap_or((1024, 1024))
    };

    let normalized_path = file_path.to_string_lossy().replace('\\', "/");
    let item = AssetItem {
        id: format!("ast_{}_{}", ts_str, short_uid),
        project: proj_clean,
        archetype: if archetype.trim().is_empty() {
            "item_icon".to_string()
        } else {
            archetype.trim().to_string()
        },
        file_path: normalized_path,
        prompt: user_prompt.to_string(),
        model: model.to_string(),
        via: via.to_string(),
        width,
        height,
        created_at: now_dt.timestamp_millis(),
    };

    state.db.insert_asset(&item)?;
    Ok(item)
}

pub fn sanitize_project_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "_all" {
        return String::new();
    }
    trimmed
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(*c, ' ' | '-' | '_' | '·'))
        .take(40)
        .collect::<String>()
        .trim()
        .to_string()
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<ProjectInfo>> {
    state.db.list_projects()
}

#[tauri::command]
pub fn upsert_project(
    state: State<'_, AppState>,
    name: String,
    style: String,
    style_prompt: String,
    reference_images: Option<Vec<String>>,
) -> AppResult<Vec<ProjectInfo>> {
    let clean = sanitize_project_name(&name);
    if clean.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "Project name cannot be empty",
        ));
    }
    let existing = state
        .db
        .list_projects()?
        .into_iter()
        .find(|p| p.name == clean);
    let refs = reference_images
        .unwrap_or_else(|| existing.map(|p| p.reference_images).unwrap_or_default());

    let dir = state.library_root.join(&clean);
    fs::create_dir_all(&dir)?;
    let meta = serde_json::json!({
        "name": clean,
        "style": style,
        "stylePrompt": style_prompt,
        "referenceImages": refs,
    });
    fs::write(
        dir.join("project.json"),
        serde_json::to_string_pretty(&meta).unwrap_or_default(),
    )?;
    state
        .db
        .upsert_project(&clean, &style, &style_prompt, &refs)?;
    state.db.list_projects()
}

#[tauri::command]
pub fn rename_project(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> AppResult<Vec<ProjectInfo>> {
    let old_clean = sanitize_project_name(&old_name);
    let new_clean = sanitize_project_name(&new_name);
    if old_clean.is_empty() || new_clean.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "Project name cannot be empty",
        ));
    }
    let src = state.library_root.join(&old_clean);
    let dst = state.library_root.join(&new_clean);
    if dst.exists() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "Target project name already exists",
        ));
    }
    if src.exists() {
        fs::rename(&src, &dst)?;
    } else {
        fs::create_dir_all(&dst)?;
    }
    state
        .db
        .rename_project(&old_clean, &new_clean, &state.library_root)?;
    state.db.list_projects()
}

#[tauri::command]
pub fn list_assets(
    state: State<'_, AppState>,
    project: Option<String>,
    limit: Option<usize>,
) -> AppResult<Vec<AssetItem>> {
    state
        .db
        .list_assets(project.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
pub fn delete_asset(state: State<'_, AppState>, id: String) -> AppResult<()> {
    if let Some(fp) = state.db.delete_asset(&id)? {
        let _ = fs::remove_file(fp);
    }
    Ok(())
}

#[tauri::command]
pub fn update_asset_archetype(
    state: State<'_, AppState>,
    id: String,
    archetype: String,
) -> AppResult<()> {
    state.db.update_asset_archetype(&id, &archetype)
}

#[tauri::command]
pub fn reveal_in_explorer(file_path: String) -> AppResult<()> {
    let win_path = file_path.replace('/', "\\");
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", win_path))
        .spawn()?;
    Ok(())
}

#[tauri::command]
pub fn export_config_bundle(state: State<'_, AppState>) -> AppResult<String> {
    let settings = get_settings(state.clone())?;
    let projects = state.db.list_projects()?;
    let mut exported_profiles = Vec::new();
    for p in &settings.profiles {
        let key = state.secrets.get_key(&p.id).unwrap_or_default();
        exported_profiles.push(serde_json::json!({
            "id": p.id,
            "name": p.name,
            "apiBase": p.api_base,
            "apiKey": key,
            "apiMode": p.api_mode,
            "apiModel": p.api_model,
            "models": p.models,
            "comfyConfig": p.comfy_config,
        }));
    }
    let bundle = serde_json::json!({
        "app": "sketch2asset",
        "version": 2,
        "profiles": exported_profiles,
        "projects": projects,
    });
    Ok(serde_json::to_string_pretty(&bundle).unwrap_or_default())
}

#[tauri::command]
pub fn import_config_bundle(state: State<'_, AppState>, raw_json: String) -> AppResult<AppSettings> {
    let val: serde_json::Value = serde_json::from_str(&raw_json)
        .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;

    if let Some(arr) = val.get("profiles").and_then(|v| v.as_array()) {
        let mut guard = state.settings.lock().unwrap();
        for item in arr {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Imported")
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
                .unwrap_or("gpt-image-1")
                .to_string();
            let models: Vec<String> = item
                .get("models")
                .and_then(|v| v.as_array())
                .map(|m| {
                    m.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let new_id = format!("prof_{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
            if !api_key.is_empty() {
                state.secrets.set_key(&new_id, api_key);
            }
            let comfy_config = item
                .get("comfyConfig")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            guard.profiles.push(ProviderProfile {
                id: new_id,
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
                comfy_config,
            });
        }
    }
    state.persist_settings()?;
    get_settings(state)
}

#[tauri::command]
pub fn list_matting_models(state: State<'_, AppState>) -> AppResult<Vec<MattingModelInfo>> {
    Ok(state.matting.list_models())
}

#[tauri::command]
pub fn delete_matting_model(
    state: State<'_, AppState>,
    model_id: String,
) -> AppResult<Vec<MattingModelInfo>> {
    state.matting.delete_model(&model_id)?;
    Ok(state.matting.list_models())
}

#[tauri::command]
pub fn open_models_folder(state: State<'_, AppState>) -> AppResult<()> {
    let dir = if state.matting.workspace_models_dir.exists() {
        state.matting.workspace_models_dir.clone()
    } else {
        fs::create_dir_all(&state.matting.appdata_models_dir)?;
        state.matting.appdata_models_dir.clone()
    };
    let win_path = dir.to_string_lossy().replace('/', "\\");
    std::process::Command::new("explorer.exe")
        .arg(win_path)
        .spawn()?;
    Ok(())
}

#[tauri::command]
pub async fn download_matting_model(
    state: State<'_, AppState>,
    model_id: String,
) -> AppResult<Vec<MattingModelInfo>> {
    let models = state.matting.list_models();
    let target = models
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "Unknown matting model ID"))?;

    if target.installed {
        return Ok(state.matting.list_models());
    }

    let dest_dir = {
        fs::create_dir_all(&state.matting.appdata_models_dir)?;
        state.matting.appdata_models_dir.clone()
    };

    let final_path = dest_dir.join(&target.file_name);
    let part_path = dest_dir.join(format!("{}.part", target.file_name));

    let dl_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()?;

    let mut last_err = String::from("No download URLs configured");
    let mut downloaded = false;

    for url in &target.urls {
        match dl_client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(bytes) if bytes.len() > 1024 * 1024 => {
                    fs::write(&part_path, &bytes)?;
                    fs::rename(&part_path, &final_path)?;
                    downloaded = true;
                    break;
                }
                Ok(_) => {
                    last_err = format!("Downloaded file from {} was unexpectedly small", url);
                }
                Err(e) => {
                    last_err = e.to_string();
                }
            },
            Ok(resp) => {
                last_err = format!("HTTP {} from {}", resp.status(), url);
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    if !downloaded {
        let _ = fs::remove_file(&part_path);
        return Err(AppError::new(
            AppErrorCode::Network,
            format!("Failed to download model '{}': {}", target.name, last_err),
        ));
    }

    Ok(state.matting.list_models())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MattingRequestPayload {
    pub image_data_url: Option<String>,
    pub image_path: Option<String>,
    pub model_id: String,
    pub device: String, // "auto" | "gpu" | "cpu"
    pub enhanced: bool,
    pub choke: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MattingResponsePayload {
    pub image_data_url: String,
    pub width: u32,
    pub height: u32,
    pub provider_info: String,
    pub elapsed_ms: u64,
}

fn decode_data_url_bytes(data_url: &str) -> AppResult<Vec<u8>> {
    if let Some(idx) = data_url.find(";base64,") {
        let raw_b64 = &data_url[idx + 8..];
        base64::engine::general_purpose::STANDARD
            .decode(raw_b64.trim())
            .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))
    } else {
        Err(AppError::new(
            AppErrorCode::InvalidInput,
            "Invalid data URL format",
        ))
    }
}

#[tauri::command]
pub async fn run_ai_matting(
    state: State<'_, AppState>,
    payload: MattingRequestPayload,
) -> AppResult<MattingResponsePayload> {
    let start_ts = std::time::Instant::now();
    let img_bytes = if let Some(ref durl) = payload
        .image_data_url
        .as_ref()
        .filter(|s| !s.trim().is_empty())
    {
        decode_data_url_bytes(durl)?
    } else if let Some(ref p) = payload.image_path.as_ref().filter(|s| !s.trim().is_empty()) {
        fs::read(p)?
    } else {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "No source image provided for AI matting",
        ));
    };

    let dyn_img = image::load_from_memory(&img_bytes)
        .map_err(|e| AppError::new(AppErrorCode::ImageDecode, e.to_string()))?;

    let engine = state.matting.clone();
    let model_id = payload.model_id.clone();
    let device = payload.device.clone();
    let enhanced = payload.enhanced;
    let choke = payload.choke;

    let (rgba_img, provider_info) = tokio::task::spawn_blocking(move || {
        engine.run_matting(&dyn_img, &model_id, &device, enhanced, choke)
    })
    .await
    .map_err(|e| AppError::new(AppErrorCode::MattingFailed, e.to_string()))??;

    let (width, height) = rgba_img.dimensions();
    let mut png_buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba_img)
        .write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(png_buf.into_inner());
    Ok(MattingResponsePayload {
        image_data_url: format!("data:image/png;base64,{}", b64),
        width,
        height,
        provider_info,
        elapsed_ms: start_ts.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
pub fn read_image_data_url(file_path: String) -> AppResult<String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err(AppError::new(
            AppErrorCode::NotFound,
            format!("File not found: {}", file_path),
        ));
    }
    let bytes = fs::read(&path)?;
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/png",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
pub fn save_studio_asset(
    state: State<'_, AppState>,
    image_data_url: String,
    project: String,
    archetype: String,
    prompt_label: Option<String>,
) -> AppResult<AssetItem> {
    let raw_bytes = decode_data_url_bytes(&image_data_url)?;
    let label = prompt_label
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Studio Processed Asset".to_string());
    save_generated_image(
        &state,
        &raw_bytes,
        &project,
        &archetype,
        &label,
        "studio",
        "studio",
        None,
    )
}

#[tauri::command]
pub fn export_studio_png(
    state: State<'_, AppState>,
    image_data_url: String,
    file_name: String,
    reveal: bool,
) -> AppResult<String> {
    let raw_bytes = decode_data_url_bytes(&image_data_url)?;
    let export_dir = dirs::download_dir().unwrap_or_else(|| state.library_root.join("_exports"));
    fs::create_dir_all(&export_dir)?;

    let safe_name: String = file_name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(*c, '_' | '-' | '.' | ' '))
        .collect();
    let final_name = if safe_name.to_lowercase().ends_with(".png") {
        safe_name
    } else {
        format!("{}.png", safe_name)
    };

    let target_path = export_dir.join(&final_name);
    fs::write(&target_path, &raw_bytes)?;

    if reveal {
        let win_path = target_path.to_string_lossy().replace('/', "\\");
        let _ = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", win_path))
            .spawn();
    }

    Ok(target_path.to_string_lossy().replace('\\', "/"))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionAnnotationItem {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionEditPayload {
    pub profile_id: String,
    pub model: String,
    pub mode: String, // "visual_guide" | "mask_edit"
    pub original_data_url: String,
    pub guide_data_url: Option<String>,
    pub mask_data_url: Option<String>,
    pub annotations: Vec<RegionAnnotationItem>,
    #[serde(default)]
    pub global_note: String,
    #[serde(default)]
    pub size: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionEditResponse {
    pub image_data_url: String,
    pub via: String,
    pub elapsed_ms: u64,
}

#[tauri::command]
pub async fn run_studio_region_edit(
    state: State<'_, AppState>,
    payload: RegionEditPayload,
) -> AppResult<RegionEditResponse> {
    let start_ts = std::time::Instant::now();

    if payload.annotations.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "At least one annotated region is required",
        ));
    }

    let (api_base, api_mode, model, comfy_cfg) = {
        let guard = state.settings.lock().unwrap();
        let prof = guard
            .profiles
            .iter()
            .find(|p| p.id == payload.profile_id)
            .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "Profile not found"))?;
        let m = if payload.model.trim().is_empty() {
            prof.api_model.clone()
        } else {
            payload.model.trim().to_string()
        };
        (
            prof.api_base.clone(),
            prof.api_mode.clone(),
            m,
            prof.comfy_config.clone(),
        )
    };

    let opt_key = state.secrets.get_key(&payload.profile_id);
    if api_mode != "comfyui" && opt_key.is_none() {
        return Err(AppError::new(
            AppErrorCode::MissingApiKey,
            "Please configure the API Key for the active profile first",
        ));
    }
    let api_key = opt_key.unwrap_or_default();

    let orig_bytes = decode_data_url_bytes(&payload.original_data_url)?;
    let guide_bytes = payload
        .guide_data_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(decode_data_url_bytes)
        .transpose()?
        .map(|b| (b, "image/png".to_string()));
    let mask_bytes = payload
        .mask_data_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(decode_data_url_bytes)
        .transpose()?
        .map(|b| (b, "image/png".to_string()));

    let mut region_lines = Vec::new();
    for item in &payload.annotations {
        let note_clean = item.note.trim();
        if !note_clean.is_empty() {
            region_lines.push(format!(
                "- Region #{} (bounding box x={}, y={}, w={}, h={}): {}",
                item.index, item.x, item.y, item.w, item.h, note_clean
            ));
        }
    }

    let mut prompt = if payload.mode == "mask_edit" {
        format!(
            "Inpaint and modify ONLY the masked regions of this game asset according to these instructions:\n{}\nKeep all unmasked areas, art style, lighting, proportions, and background strictly identical.",
            region_lines.join("\n")
        )
    } else {
        format!(
            "Precision Game Asset Region Edit Task:\nImage 1 is the original clean game asset. Image 2 shows numbered colored bounding boxes (#1, #2, ...) marking the exact local regions to modify.\nModify ONLY the numbered regions according to the instructions below, while keeping all unmarked areas, character/object proportions, art style, lighting, and background strictly identical to Image 1:\n{}\nCRITICAL OUTPUT RULE: Output ONLY the final modified clean asset image WITHOUT any colored bounding boxes, border lines, or number tags (#1, #2).",
            region_lines.join("\n")
        )
    };

    if !payload.global_note.trim().is_empty() {
        prompt.push_str(&format!(
            "\nAdditional style/consistency requirement: {}",
            payload.global_note.trim()
        ));
    }

    let size = if payload.size.trim().is_empty() {
        "1024x1024".to_string()
    } else {
        payload.size.trim().to_string()
    };

    let (out_bytes, via) = if api_mode == "comfyui" {
        providers::comfyui::generate_via_comfyui(
            &state.http,
            &api_base,
            if api_key.is_empty() { None } else { Some(&api_key) },
            &model,
            &prompt,
            "",
            &size,
            Some((orig_bytes, "image/png".to_string())),
            if payload.mode == "mask_edit" {
                mask_bytes
            } else {
                guide_bytes
            },
            comfy_cfg.as_ref(),
        )
        .await?
    } else if api_mode == "chat" && payload.mode != "mask_edit" {
        providers::edit_regions_via_chat(
            &state.http,
            &api_base,
            &api_key,
            &model,
            &prompt,
            (orig_bytes, "image/png".to_string()),
            guide_bytes,
        )
        .await?
    } else {
        providers::edit_regions_via_images(
            &state.http,
            &api_base,
            &api_key,
            &model,
            &prompt,
            &size,
            (orig_bytes, "image/png".to_string()),
            guide_bytes,
            if payload.mode == "mask_edit" {
                mask_bytes
            } else {
                None
            },
        )
        .await?
    };

    // Normalize output to PNG data URL
    let png_bytes = if let Ok(img) = image::load_from_memory(&out_bytes) {
        let mut cursor = std::io::Cursor::new(Vec::new());
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
        cursor.into_inner()
    } else {
        out_bytes
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Ok(RegionEditResponse {
        image_data_url: format!("data:image/png;base64,{}", b64),
        via,
        elapsed_ms: start_ts.elapsed().as_millis() as u64,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterGeneratePayload {
    pub profile_id: String,
    pub model: String,
    pub sub_mode: String, // "turnaround" | "outfit" | "spritesheet"
    pub prompt: String,
    pub style_prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    pub size: String,
    pub project: String,
    pub archetype: String,
    pub anchor_data_url: Option<String>,
    pub secondary_ref_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterGenerateResponse {
    pub asset: AssetItem,
    pub image_data_url: String,
    pub via: String,
    pub elapsed_ms: u64,
}

#[tauri::command]
pub async fn generate_character_asset(
    state: State<'_, AppState>,
    payload: CharacterGeneratePayload,
) -> AppResult<CharacterGenerateResponse> {
    let start_ts = std::time::Instant::now();

    let (api_base, api_mode, model, comfy_cfg) = {
        let guard = state.settings.lock().unwrap();
        let prof = guard
            .profiles
            .iter()
            .find(|p| p.id == payload.profile_id)
            .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "Profile not found"))?;
        let m = if payload.model.trim().is_empty() {
            prof.api_model.clone()
        } else {
            payload.model.trim().to_string()
        };
        (
            prof.api_base.clone(),
            prof.api_mode.clone(),
            m,
            prof.comfy_config.clone(),
        )
    };

    let opt_key = state.secrets.get_key(&payload.profile_id);
    if api_mode != "comfyui" && opt_key.is_none() {
        return Err(AppError::new(
            AppErrorCode::MissingApiKey,
            "Please configure the API Key for the active profile first",
        ));
    }
    let api_key = opt_key.unwrap_or_default();

    let anchor_bytes = payload
        .anchor_data_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(decode_data_url_bytes)
        .transpose()?
        .map(|b| (b, "image/png".to_string()));

    let secondary_bytes = payload
        .secondary_ref_data_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(decode_data_url_bytes)
        .transpose()?
        .map(|b| (b, "image/png".to_string()));

    let mut prompt_parts = Vec::new();
    if !payload.prompt.trim().is_empty() {
        prompt_parts.push(payload.prompt.trim().to_string());
    }
    if !payload.style_prompt.trim().is_empty() {
        prompt_parts.push(payload.style_prompt.trim().to_string());
    }
    prompt_parts.push(
        "CRITICAL BACKGROUND RULE: Pure solid flat studio white background (#FFFFFF) only, completely uniform with zero vignette, zero backdrop gradients, zero floor plane, zero ground cast shadow, and no text labels or watermarks.".to_string(),
    );
    if !payload.negative_prompt.trim().is_empty() {
        prompt_parts.push(format!(
            "STRICT AVOID / NEGATIVE CONSTRAINTS: {}",
            payload.negative_prompt.trim()
        ));
    }
    let final_prompt = prompt_parts.join("\n\n");

    let size = if payload.size.trim().is_empty() {
        "1024x1024".to_string()
    } else {
        payload.size.trim().to_string()
    };

    let (raw_bytes, via) = if api_mode == "comfyui" {
        providers::comfyui::generate_via_comfyui(
            &state.http,
            &api_base,
            if api_key.is_empty() { None } else { Some(&api_key) },
            &model,
            &final_prompt,
            &payload.negative_prompt,
            &size,
            anchor_bytes,
            secondary_bytes,
            comfy_cfg.as_ref(),
        )
        .await?
    } else if let Some(anchor) = anchor_bytes {
        if secondary_bytes.is_some() {
            // Dual-reference mode (Identity Anchor + Outfit/Pose Reference)
            if api_mode == "chat" {
                providers::edit_regions_via_chat(
                    &state.http,
                    &api_base,
                    &api_key,
                    &model,
                    &final_prompt,
                    anchor,
                    secondary_bytes,
                )
                .await?
            } else {
                providers::edit_regions_via_images(
                    &state.http,
                    &api_base,
                    &api_key,
                    &model,
                    &final_prompt,
                    &size,
                    anchor,
                    secondary_bytes,
                    None,
                )
                .await?
            }
        } else if api_mode == "chat" {
            providers::generate_via_chat(
                &state.http,
                &api_base,
                &api_key,
                &model,
                &final_prompt,
                Some(anchor),
            )
            .await?
        } else {
            providers::generate_via_images(
                &state.http,
                &api_base,
                &api_key,
                &model,
                &final_prompt,
                &size,
                Some(anchor),
            )
            .await?
        }
    } else if api_mode == "chat" {
        providers::generate_via_chat(
            &state.http,
            &api_base,
            &api_key,
            &model,
            &final_prompt,
            None,
        )
        .await?
    } else {
        providers::generate_via_images(
            &state.http,
            &api_base,
            &api_key,
            &model,
            &final_prompt,
            &size,
            None,
        )
        .await?
    };

    let arch = if payload.archetype.trim().is_empty() {
        "character_card"
    } else {
        payload.archetype.trim()
    };

    let asset = save_generated_image(
        &state,
        &raw_bytes,
        &payload.project,
        arch,
        &format!("[{}] {}", payload.sub_mode.to_uppercase(), payload.prompt.trim()),
        &model,
        &via,
        None,
    )?;

    let png_data_url = read_image_data_url(asset.file_path.clone())?;

    Ok(CharacterGenerateResponse {
        asset,
        image_data_url: png_data_url,
        via,
        elapsed_ms: start_ts.elapsed().as_millis() as u64,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
    pub release_notes: String,
    pub published_at: String,
}

#[tauri::command]
pub async fn check_app_update(state: State<'_, AppState>) -> AppResult<AppUpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = &state.http;
    let url = "https://api.github.com/repos/kbkkb/Sketch2Asset/releases/latest";

    let resp = match client
        .get(url)
        .header("User-Agent", "Sketch2Asset-Desktop")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return Ok(AppUpdateInfo {
                current_version: current_version.clone(),
                latest_version: current_version,
                has_update: false,
                release_url: "https://github.com/kbkkb/Sketch2Asset/releases".to_string(),
                release_notes: String::new(),
                published_at: String::new(),
            });
        }
    };

    let json: serde_json::Value = resp.json().await.unwrap_or_default();

    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let release_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://github.com/kbkkb/Sketch2Asset/releases")
        .to_string();
    let release_notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let published_at = json
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let clean_tag = tag_name.trim_start_matches('v').trim();
    let clean_current = current_version.trim_start_matches('v').trim();

    let has_update = !clean_tag.is_empty() && clean_tag != clean_current;

    Ok(AppUpdateInfo {
        current_version,
        latest_version: if clean_tag.is_empty() {
            "0.2.0".to_string()
        } else {
            tag_name
        },
        has_update,
        release_url,
        release_notes,
        published_at,
    })
}

#[tauri::command]
pub fn get_agent_hub_status(
    state: State<'_, AppState>,
) -> AppResult<crate::mcp::installer::AgentHubStatus> {
    crate::mcp::installer::get_hub_status(&state.app_data_dir)
}

#[tauri::command]
pub fn install_agent_integration(
    state: State<'_, AppState>,
    agent_id: String,
) -> AppResult<crate::mcp::installer::AgentHubStatus> {
    crate::mcp::installer::install_for_agent(&state.app_data_dir, &agent_id)
}

#[tauri::command]
pub fn uninstall_agent_integration(
    state: State<'_, AppState>,
    agent_id: String,
) -> AppResult<crate::mcp::installer::AgentHubStatus> {
    crate::mcp::installer::uninstall_for_agent(&state.app_data_dir, &agent_id)
}

#[tauri::command]
pub fn export_agent_skill_bundle(
    state: State<'_, AppState>,
    reveal: bool,
) -> AppResult<String> {
    crate::mcp::installer::export_skill_bundle(&state.app_data_dir, reveal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload(sketch_path: Option<&str>, sketch_data_url: Option<&str>) -> GeneratePayload {
        GeneratePayload {
            profile_id: "default".to_string(),
            model: "gpt-image-1".to_string(),
            prompt: "silver knight running".to_string(),
            archetype: "character_card".to_string(),
            archetype_prompt: "Full-Body Game Character".to_string(),
            style_prompt: "hand-painted fantasy".to_string(),
            project_style_prompt: "".to_string(),
            background_prompt: "Pure flat #FFFFFF white backdrop, zero vignette.".to_string(),
            negative_prompt: "no text".to_string(),
            size: "1536x1024".to_string(),
            project: "test".to_string(),
            count: 1,
            sketch_data_url: sketch_data_url.map(|s| s.to_string()),
            sketch_path: sketch_path.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_resolve_sketch_input_missing_path_returns_not_found() {
        let missing = std::env::temp_dir().join(format!(
            "sketch2asset_missing_{}.png",
            uuid::Uuid::new_v4().simple()
        ));
        let payload = sample_payload(Some(&missing.to_string_lossy()), None);
        let err = resolve_sketch_input(&payload)
            .expect_err("non-existent sketch_path must fail with NotFound instead of falling back");
        assert_eq!(err.code, AppErrorCode::NotFound);
        assert!(err.message.contains("not found"), "msg={}", err.message);
    }

    #[test]
    fn test_resolve_sketch_input_unreadable_directory_returns_not_found() {
        let dir = std::env::temp_dir();
        let payload = sample_payload(Some(&dir.to_string_lossy()), None);
        let err = resolve_sketch_input(&payload)
            .expect_err("directory path cannot be read as image file");
        assert_eq!(err.code, AppErrorCode::NotFound);
    }

    #[test]
    fn test_resolve_sketch_input_existing_file_loads_bytes_and_mime() {
        let tmp_file = std::env::temp_dir().join(format!(
            "sketch2asset_ref_{}.jpg",
            uuid::Uuid::new_v4().simple()
        ));
        let expected = b"fake-jpeg-data";
        fs::write(&tmp_file, expected).unwrap();

        let payload = sample_payload(Some(&format!("  {}  ", tmp_file.display())), None);
        let res = resolve_sketch_input(&payload).unwrap();
        let _ = fs::remove_file(&tmp_file);

        let (bytes, mime) = res.expect("expected resolved sketch bytes");
        assert_eq!(bytes, expected);
        assert_eq!(mime, "image/jpeg");
    }

    #[test]
    fn test_resolve_sketch_input_blank_or_none_returns_ok_none() {
        let payload_none = sample_payload(None, None);
        assert!(resolve_sketch_input(&payload_none).unwrap().is_none());

        let payload_blank = sample_payload(Some("   "), Some("  "));
        assert!(resolve_sketch_input(&payload_blank).unwrap().is_none());
    }

    #[test]
    fn test_resolve_sketch_input_invalid_data_url_returns_invalid_input() {
        let payload = sample_payload(None, Some("not-a-valid-data-url"));
        let err = resolve_sketch_input(&payload).expect_err("malformed data URL must fail");
        assert_eq!(err.code, AppErrorCode::InvalidInput);
    }

    #[test]
    fn test_assemble_game_asset_prompt_prefixes_background_and_sketch() {
        let payload = sample_payload(None, None);
        let prompt = assemble_game_asset_prompt(&payload, true);
        assert!(prompt.contains("Based on the provided reference/sketch"));
        assert!(prompt.contains("CRITICAL BACKGROUND RULE: Pure flat #FFFFFF white backdrop"));
        assert!(prompt.contains("Target Canvas Aspect Ratio & Resolution: 3:2 horizontal landscape (1536x1024px)."));
        assert!(prompt.contains("STRICT AVOID / NEGATIVE CONSTRAINTS: no text"));
    }
}
