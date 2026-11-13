pub mod installer;

use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use crate::commands::{
    assemble_game_asset_prompt, fit_image_to_target_size, resolve_sketch_input,
    sanitize_project_name, save_generated_image, GeneratePayload,
};
use crate::error::{AppError, AppErrorCode, AppResult};
use crate::postprocess;
use crate::providers;
use crate::state::AppState;

pub fn init_standalone_state() -> AppResult<AppState> {
    let app_data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.sketch2asset.app");

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

    AppState::initialize(app_data_dir, workspace_dir)
}

fn default_archetype_prompt(arch: &str) -> &'static str {
    match arch {
        "item_icon" => "Game Item Icon: Single isolated object centered on canvas, clear readable silhouette, generous margin around edges, no cropped parts.",
        "pixel_sprite" => "Authentic Pixel Art Sprite: Strict 1:1 square pixel grid, flat solid colors per cell, zero sub-pixel anti-aliasing or mixels, clean 1px dark outline.",
        "character_card" => "Full-Body Game Character Concept: Complete head-to-toe standing figure, clear silhouette and costume layering.",
        "tile_texture" => "Orthographic Seamless Game Tile Texture: 90-degree top-down flat diffuse lighting, repeatable surface pattern.",
        "ui_element" => "Game UI Component / Frame: Symmetrical front-facing orthographic UI element, clean center area.",
        _ => "High quality 2D game-ready digital asset, crisp edges and clean silhouette.",
    }
}

async fn exec_status(state: &AppState) -> AppResult<Value> {
    state.reload_settings_from_disk();
    let settings = state.settings.lock().unwrap().clone();
    let active_prof = settings
        .profiles
        .iter()
        .find(|p| p.id == settings.active_profile_id)
        .cloned();
    let has_key = active_prof
        .as_ref()
        .map(|p| state.secrets.has_key(&p.id) || p.api_mode == "comfyui")
        .unwrap_or(false);
    let all_profiles: Vec<Value> = settings
        .profiles
        .iter()
        .map(|p| {
            let hk = state.secrets.has_key(&p.id) || p.api_mode == "comfyui";
            json!({
                "id": p.id,
                "name": p.name,
                "apiBase": p.api_base,
                "apiMode": p.api_mode,
                "apiModel": p.api_model,
                "enabledModels": p.models,
                "hasApiKey": hk,
                "isActive": p.id == settings.active_profile_id
            })
        })
        .collect();
    let projects = state.db.list_projects().unwrap_or_default();
    let matting_models = state.matting.list_models();

    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "appDataDir": state.app_data_dir,
        "libraryRoot": state.library_root,
        "activeProject": settings.active_project,
        "activeProfileId": settings.active_profile_id,
        "activeProfile": active_prof.map(|p| json!({
            "id": p.id,
            "name": p.name,
            "apiBase": p.api_base,
            "apiMode": p.api_mode,
            "apiModel": p.api_model,
            "enabledModels": p.models,
            "hasApiKey": has_key
        })),
        "profiles": all_profiles,
        "projects": projects,
        "mattingModels": matting_models
    }))
}

async fn exec_create_project(state: &AppState, args: &Value) -> AppResult<Value> {
    state.reload_settings_from_disk();
    let raw_name = args
        .get("name")
        .or_else(|| args.get("project"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let clean = sanitize_project_name(raw_name);
    if clean.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "Project 'name' is required and cannot be empty",
        ));
    }

    let style = args
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    let style_prompt = args
        .get("style_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let set_active = args
        .get("set_active")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let existing = state
        .db
        .list_projects()?
        .into_iter()
        .find(|p| p.name == clean);
    let refs = existing.map(|p| p.reference_images).unwrap_or_default();

    let dir = state.library_root.join(&clean);
    fs::create_dir_all(&dir)?;
    let meta = json!({
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

    if set_active {
        {
            let mut guard = state.settings.lock().unwrap();
            guard.active_project = clean.clone();
        }
        state.persist_settings()?;
    }

    let projects = state.db.list_projects()?;
    Ok(json!({
        "createdOrUpdatedProject": clean,
        "activeProject": state.settings.lock().unwrap().active_project,
        "projects": projects
    }))
}

async fn exec_switch_profile(state: &AppState, args: &Value) -> AppResult<Value> {
    state.reload_settings_from_disk();
    let target_profile = args
        .get("profile_id")
        .or_else(|| args.get("profile"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let target_model = args
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let target_project = args
        .get("active_project")
        .or_else(|| args.get("project"))
        .and_then(|v| v.as_str())
        .map(|s| sanitize_project_name(s));

    {
        let mut guard = state.settings.lock().unwrap();

        if let Some(ref prof_query) = target_profile {
            let q_lower = prof_query.to_lowercase();
            let found_id = guard
                .profiles
                .iter()
                .find(|p| {
                    p.id.eq_ignore_ascii_case(prof_query)
                        || p.name.eq_ignore_ascii_case(prof_query)
                        || p.name.to_lowercase().contains(&q_lower)
                })
                .map(|p| p.id.clone())
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::NotFound,
                        format!("No provider profile matching '{}'", prof_query),
                    )
                })?;
            guard.active_profile_id = found_id;
        }

        if let Some(ref m) = target_model {
            let active_id = guard.active_profile_id.clone();
            if let Some(prof) = guard.profiles.iter_mut().find(|p| p.id == active_id) {
                prof.api_model = m.clone();
                if !prof.models.contains(m) {
                    prof.models.push(m.clone());
                }
            }
        }

        if let Some(proj) = target_project {
            guard.active_project = proj;
        }
    }

    state.persist_settings()?;
    exec_status(state).await
}

async fn exec_generate_asset(state: &AppState, args: &Value) -> AppResult<Value> {
    state.reload_settings_from_disk();
    let prompt = args
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if prompt.is_empty() {
        return Err(AppError::new(AppErrorCode::InvalidInput, "prompt is required"));
    }

    let settings = state.settings.lock().unwrap().clone();
    let profile_query = args
        .get("profile_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&settings.active_profile_id);

    let prof = settings
        .profiles
        .iter()
        .find(|p| {
            p.id.eq_ignore_ascii_case(profile_query)
                || p.name.eq_ignore_ascii_case(profile_query)
                || p.name
                    .to_lowercase()
                    .contains(&profile_query.to_lowercase())
        })
        .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "Active provider profile not found"))?
        .clone();
    let profile_id = prof.id.clone();

    let model = args
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if !prof.api_model.trim().is_empty() {
                prof.api_model.clone()
            } else {
                prof.models.first().cloned().unwrap_or_default()
            }
        });

    let opt_key = state.secrets.get_key(&profile_id);
    if prof.api_mode != "comfyui" && opt_key.is_none() {
        return Err(AppError::new(
            AppErrorCode::MissingApiKey,
            "API Key not configured in Sketch2Asset. Please set it in Sketch2Asset Settings first.",
        ));
    }
    let api_key = opt_key.unwrap_or_default();

    let archetype = args
        .get("archetype")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("item_icon")
        .to_string();

    let size = args
        .get("size")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("1024x1024")
        .to_string();

    let project = args
        .get("project")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&settings.active_project)
        .to_string();

    let style_prompt = args
        .get("style_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let negative_prompt = args
        .get("negative_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("no text, no watermark, no cropped edges, no cast ground shadow, no backdrop vignette, no gradient background")
        .to_string();

    let sketch_path = args
        .get("sketch_path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let align_spritesheet = args
        .get("align_spritesheet")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let sprite_rows = args
        .get("sprite_rows")
        .or_else(|| args.get("rows"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let sprite_cols = args
        .get("sprite_cols")
        .or_else(|| args.get("cols"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let export_frames = args
        .get("export_frames")
        .and_then(|v| v.as_bool())
        .unwrap_or(align_spritesheet);

    let project_style_prompt = state
        .db
        .list_projects()
        .ok()
        .and_then(|list| list.into_iter().find(|p| p.name == project))
        .map(|p| p.style_prompt)
        .unwrap_or_default();

    let is_pixel_art = args
        .get("pixel_perfect")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| {
            postprocess::is_pixel_art_requested(
                &archetype,
                &prompt,
                &format!("{} {}", style_prompt, project_style_prompt),
            )
        });
    let pixel_block_size = args
        .get("pixel_block_size")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    // Default auto_remove_bg to true for sprite sheets and pixel sprites unless explicitly passed as false
    let auto_remove_bg = args
        .get("auto_remove_bg")
        .and_then(|v| v.as_bool())
        .unwrap_or(align_spritesheet || is_pixel_art || archetype == "pixel_sprite");

    let output_path = args
        .get("output_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from);

    let background_prompt = resolve_mcp_background_prompt(&archetype, auto_remove_bg);

    let payload = GeneratePayload {
        profile_id,
        model: model.clone(),
        prompt: prompt.clone(),
        archetype: archetype.clone(),
        archetype_prompt: default_archetype_prompt(&archetype).to_string(),
        style_prompt,
        project_style_prompt,
        background_prompt: background_prompt.to_string(),
        negative_prompt,
        size: size.clone(),
        project: project.clone(),
        count: 1,
        sketch_data_url: None,
        sketch_path,
    };

    let sketch_bytes = resolve_sketch_input(&payload)?;
    let final_prompt = assemble_game_asset_prompt(&payload, sketch_bytes.is_some());

    let (mut img_bytes, via) = if prof.api_mode == "comfyui" {
        providers::comfyui::generate_via_comfyui(
            &state.http,
            &prof.api_base,
            if api_key.is_empty() { None } else { Some(&api_key) },
            &model,
            &final_prompt,
            &payload.negative_prompt,
            &size,
            sketch_bytes,
            None,
            prof.comfy_config.as_ref(),
        )
        .await?
    } else if prof.api_mode == "chat" {
        providers::generate_via_chat(
            &state.http,
            &prof.api_base,
            &api_key,
            &model,
            &final_prompt,
            sketch_bytes,
        )
        .await?
    } else {
        providers::generate_via_images(
            &state.http,
            &prof.api_base,
            &api_key,
            &model,
            &final_prompt,
            &size,
            sketch_bytes,
        )
        .await?
    };

    let mut matting_provider = None;
    let mut sprite_grid_meta = None;
    let mut aligned_frames_out: Vec<image::RgbaImage> = Vec::new();
    let mut native_1x_out: Option<image::RgbaImage> = None;
    let mut detected_block_size: Option<u32> = None;
    let mut skip_resize_on_save = false;

    if let Ok(mut dyn_img) = image::load_from_memory(&img_bytes) {
        if let Some((tw, th)) = providers::parse_size_dims(&size) {
            if dyn_img.width() != tw || dyn_img.height() != th {
                dyn_img = fit_image_to_target_size(dyn_img, tw, th);
            }
        }

        let sprite_grid = if align_spritesheet {
            Some(postprocess::infer_sprite_grid(
                &prompt,
                sprite_rows,
                sprite_cols,
            ))
        } else {
            None
        };

        let mut rgba_img = if auto_remove_bg {
            let matting_res = match sprite_grid {
                Some((rows, cols)) if rows.saturating_mul(cols) > 1 => {
                    state.matting.run_matting_spritesheet_grid(
                        &dyn_img, rows, cols, "birefnet", "auto", true, 1,
                    )
                }
                _ => state
                    .matting
                    .run_matting(&dyn_img, "birefnet", "auto", true, 1),
            };
            match matting_res {
                Ok((rgba, prov)) => {
                    matting_provider = Some(prov);
                    rgba
                }
                Err(err) => {
                    eprintln!("[mcp] matting skipped due to error: {}", err);
                    dyn_img.to_rgba8()
                }
            }
        } else {
            dyn_img.to_rgba8()
        };

        if let Some((rows, cols)) = sprite_grid {
            let preserve_vertical_arc = args
                .get("preserve_vertical_arc")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| postprocess::is_airborne_motion(&prompt));
            let (aligned_sheet, aligned_frames) =
                postprocess::align_spritesheet_uniform_grid_with_options(
                    &rgba_img,
                    rows,
                    cols,
                    0.86,
                    preserve_vertical_arc,
                );

            if is_pixel_art {
                let (q_sheet, n_sheet, q_frames, blk) = postprocess::quantize_spritesheet_frames(
                    &aligned_sheet,
                    &aligned_frames,
                    rows,
                    cols,
                    pixel_block_size,
                );
                rgba_img = q_sheet;
                aligned_frames_out = q_frames;
                native_1x_out = Some(n_sheet);
                detected_block_size = Some(blk);
            } else {
                rgba_img = aligned_sheet;
                aligned_frames_out = aligned_frames;
            }

            let fw = aligned_frames_out
                .first()
                .map(|f| f.width())
                .unwrap_or(rgba_img.width() / cols.max(1));
            let fh = aligned_frames_out
                .first()
                .map(|f| f.height())
                .unwrap_or(rgba_img.height() / rows.max(1));

            sprite_grid_meta = Some(json!({
                "rows": rows,
                "cols": cols,
                "frameCount": rows * cols,
                "frameWidth": fw,
                "frameHeight": fh,
                "baselineRatio": 0.86,
                "preserveVerticalArc": preserve_vertical_arc
            }));
            skip_resize_on_save = true;
        } else if is_pixel_art {
            let (q_img, n_img, blk) =
                postprocess::quantize_pixel_art_grid(&rgba_img, pixel_block_size, true);
            rgba_img = q_img;
            native_1x_out = Some(n_img);
            detected_block_size = Some(blk);
            skip_resize_on_save = true;
        }

        let mut cursor = io::Cursor::new(Vec::new());
        if image::DynamicImage::ImageRgba8(rgba_img)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .is_ok()
        {
            img_bytes = cursor.into_inner();
        }
    }

    let asset = save_generated_image(
        state,
        &img_bytes,
        &project,
        &archetype,
        &prompt,
        &model,
        &via,
        if skip_resize_on_save {
            None
        } else {
            Some(&size)
        },
    )?;

    let base_target_path = output_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(&asset.file_path));
    let base_stem = base_target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("asset")
        .to_string();
    let base_parent = base_target_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| state.library_root.clone());

    let mut exported_to = None;
    if let Some(out) = output_path {
        if let Some(parent) = out.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::copy(&asset.file_path, &out)?;
        exported_to = Some(out.to_string_lossy().replace('\\', "/"));
    }

    let mut native_1x_path = None;
    if let Some(n_img) = native_1x_out {
        let p1x = base_parent.join(format!("{}_native_1x.png", base_stem));
        if n_img.save_with_format(&p1x, image::ImageFormat::Png).is_ok() {
            native_1x_path = Some(p1x.to_string_lossy().replace('\\', "/"));
        }
    }

    let mut exported_frames = Vec::new();
    if export_frames && !aligned_frames_out.is_empty() {
        let frames_dir = base_parent.join(format!("{}_frames", base_stem));
        if fs::create_dir_all(&frames_dir).is_ok() {
            for (idx, frame_img) in aligned_frames_out.iter().enumerate() {
                let fp = frames_dir.join(format!("frame_{:02}.png", idx + 1));
                if frame_img.save_with_format(&fp, image::ImageFormat::Png).is_ok() {
                    exported_frames.push(fp.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }

    Ok(json!({
        "asset": asset,
        "exportedTo": exported_to,
        "autoRemovedBackground": matting_provider.is_some(),
        "mattingProvider": matting_provider,
        "spriteGrid": sprite_grid_meta,
        "exportedFrames": if exported_frames.is_empty() { Value::Null } else { json!(exported_frames) },
        "pixelPerfect": is_pixel_art,
        "pixelBlockSize": detected_block_size,
        "native1xPath": native_1x_path
    }))
}

async fn exec_remove_background(state: &AppState, args: &Value) -> AppResult<Value> {
    let input_path = args
        .get("input_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if input_path.is_empty() || !Path::new(input_path).exists() {
        return Err(AppError::new(
            AppErrorCode::NotFound,
            format!("Input image not found: {}", input_path),
        ));
    }

    let model_id = args
        .get("model_id")
        .and_then(|v| v.as_str())
        .unwrap_or("birefnet");
    let device = args.get("device").and_then(|v| v.as_str()).unwrap_or("auto");
    let enhanced = args.get("enhanced").and_then(|v| v.as_bool()).unwrap_or(true);
    let choke = args.get("choke").and_then(|v| v.as_u64()).unwrap_or(1) as u8;

    let raw_bytes = fs::read(input_path)?;
    let dyn_img = image::load_from_memory(&raw_bytes)
        .map_err(|e| AppError::new(AppErrorCode::ImageDecode, e.to_string()))?;

    let (rgba, provider_info) = state
        .matting
        .run_matting(&dyn_img, model_id, device, enhanced, choke)?;
    let (width, height) = rgba.dimensions();

    let mut png_buf = io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    let out_bytes = png_buf.into_inner();

    let output_path = if let Some(out) = args
        .get("output_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
    {
        PathBuf::from(out)
    } else {
        let p = Path::new(input_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("cutout");
        p.with_file_name(format!("{}_nobg.png", stem))
    };

    if let Some(parent) = output_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&output_path, &out_bytes)?;

    Ok(json!({
        "inputPath": input_path,
        "outputPath": output_path.to_string_lossy().replace('\\', "/"),
        "width": width,
        "height": height,
        "providerInfo": provider_info
    }))
}

fn resolve_mcp_background_prompt(archetype: &str, auto_remove_bg: bool) -> &'static str {
    if archetype == "tile_texture" && !auto_remove_bg {
        "CRITICAL BACKGROUND RULE: Full-bleed seamless edge-to-edge orthographic surface texture filling the entire canvas with uniform flat lighting, no white borders or frames."
    } else {
        "CRITICAL BACKGROUND RULE: Pure flat solid studio white background (#FFFFFF) only, completely uniform with zero vignette, zero radial or studio gradients, zero floor plane, zero ground cast shadow, and no environment clutter."
    }
}

fn build_character_generate_args(args: &Value) -> AppResult<Value> {
    let sub_mode = args
        .get("sub_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("spritesheet");
    let user_prompt = args
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if user_prompt.is_empty() {
        return Err(AppError::new(AppErrorCode::InvalidInput, "prompt is required"));
    }

    let explicit_rows = args.get("rows").and_then(|v| v.as_u64()).map(|v| v as u32);
    let explicit_cols = args.get("cols").and_then(|v| v.as_u64()).map(|v| v as u32);

    let mut merged = serde_json::Map::new();
    if sub_mode == "spritesheet" {
        let (rows, cols) = postprocess::infer_sprite_grid(user_prompt, explicit_rows, explicit_cols);
        let choreo_prompt = postprocess::build_keyframe_choreography_prompt(user_prompt, rows, cols);
        merged.insert("prompt".to_string(), Value::String(choreo_prompt));
        merged.insert("align_spritesheet".to_string(), Value::Bool(true));
        merged.insert("sprite_rows".to_string(), Value::Number(rows.into()));
        merged.insert("sprite_cols".to_string(), Value::Number(cols.into()));
    } else {
        let mode_prefix = match sub_mode {
            "turnaround" => "3-View Character Turnaround Sheet: Horizontally aligned Front view, Side view, and Back view of the exact same character standing on a single shared ground baseline, pure flat #FFFFFF white background (zero vignette, zero gradient, zero cast shadow).",
            "outfit" => "Identity-Locked Character Outfit Skinning: Keep the character's facial features, hair, and body proportions strictly identical while changing costume/armor as requested, pure flat #FFFFFF white background (zero vignette, zero gradient, zero cast shadow).",
            "expression" => "2x2 Character Expression Sheet Grid: 4 distinct facial expressions of the exact same character in a clean 2x2 grid on a pure flat #FFFFFF white background (zero vignette, zero gradient, zero cast shadow).",
            _ => "2D Game Sprite Sheet Animation Sequence: Equally spaced sequential animation frames arranged in a clean horizontal/grid layout on a shared ground baseline, pure flat #FFFFFF white background (zero vignette, zero gradient, zero cast shadow).",
        };
        merged.insert(
            "prompt".to_string(),
            Value::String(format!("{}\nSubject & Action: {}", mode_prefix, user_prompt)),
        );
    }

    merged.insert("archetype".to_string(), Value::String("character_card".to_string()));
    if let Some(anchor) = args
        .get("anchor_image_path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        merged.insert("sketch_path".to_string(), Value::String(anchor.to_string()));
    }
    if let Some(sz) = args.get("size") {
        merged.insert("size".to_string(), sz.clone());
    } else {
        merged.insert("size".to_string(), Value::String("1536x1024".to_string()));
    }
    for passthrough_key in [
        "project",
        "profile_id",
        "model",
        "style_prompt",
        "negative_prompt",
        "auto_remove_bg",
        "pixel_perfect",
        "pixel_block_size",
        "export_frames",
        "output_path",
    ] {
        if let Some(val) = args.get(passthrough_key) {
            merged.insert(passthrough_key.to_string(), val.clone());
        }
    }

    Ok(Value::Object(merged))
}

async fn exec_generate_character(state: &AppState, args: &Value) -> AppResult<Value> {
    let merged = build_character_generate_args(args)?;
    exec_generate_asset(state, &merged).await
}

async fn exec_list_assets(state: &AppState, args: &Value) -> AppResult<Value> {
    state.reload_settings_from_disk();
    let project = args.get("project").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let items = state.db.list_assets(project, limit)?;
    Ok(json!({ "assets": items }))
}

async fn exec_edit_region(state: &AppState, args: &Value) -> AppResult<Value> {
    state.reload_settings_from_disk();
    let input_path = args
        .get("input_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if input_path.is_empty() || !Path::new(input_path).exists() {
        return Err(AppError::new(
            AppErrorCode::NotFound,
            format!("Input image not found: {}", input_path),
        ));
    }
    let instruction = args
        .get("instruction")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if instruction.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "instruction is required",
        ));
    }
    let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
    let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
    let w = args.get("w").and_then(|v| v.as_i64()).unwrap_or(256);
    let h = args.get("h").and_then(|v| v.as_i64()).unwrap_or(256);

    let settings = state.settings.lock().unwrap().clone();
    let profile_query = args
        .get("profile_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&settings.active_profile_id);
    let prof = settings
        .profiles
        .iter()
        .find(|p| {
            p.id.eq_ignore_ascii_case(profile_query)
                || p.name.eq_ignore_ascii_case(profile_query)
                || p.name
                    .to_lowercase()
                    .contains(&profile_query.to_lowercase())
        })
        .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "Active provider profile not found"))?
        .clone();
    let model = args
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if !prof.api_model.trim().is_empty() {
                prof.api_model.clone()
            } else {
                prof.models.first().cloned().unwrap_or_default()
            }
        });
    let opt_key = state.secrets.get_key(&prof.id);
    if prof.api_mode != "comfyui" && opt_key.is_none() {
        return Err(AppError::new(
            AppErrorCode::MissingApiKey,
            "API Key not configured",
        ));
    }
    let api_key = opt_key.unwrap_or_default();

    let orig_bytes = fs::read(input_path)?;
    let prompt = format!(
        "Precision Game Asset Region Edit Task:\nModify ONLY the rectangular region (x={}, y={}, width={}, height={}) according to this instruction: {}\nKeep all other areas, proportions, art style, lighting, and background strictly identical.",
        x, y, w, h, instruction
    );

    let (out_bytes, via) = if prof.api_mode == "comfyui" {
        providers::comfyui::generate_via_comfyui(
            &state.http,
            &prof.api_base,
            if api_key.is_empty() { None } else { Some(&api_key) },
            &model,
            &prompt,
            "",
            "1024x1024",
            Some((orig_bytes, "image/png".to_string())),
            None,
            prof.comfy_config.as_ref(),
        )
        .await?
    } else if prof.api_mode == "chat" {
        providers::edit_regions_via_chat(
            &state.http,
            &prof.api_base,
            &api_key,
            &model,
            &prompt,
            (orig_bytes, "image/png".to_string()),
            None,
        )
        .await?
    } else {
        providers::edit_regions_via_images(
            &state.http,
            &prof.api_base,
            &api_key,
            &model,
            &prompt,
            "1024x1024",
            (orig_bytes, "image/png".to_string()),
            None,
            None,
        )
        .await?
    };

    let output_path = if let Some(out) = args
        .get("output_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
    {
        PathBuf::from(out)
    } else {
        let p = Path::new(input_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("edited");
        p.with_file_name(format!("{}_edited.png", stem))
    };

    if let Some(parent) = output_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&output_path, &out_bytes)?;

    Ok(json!({
        "inputPath": input_path,
        "outputPath": output_path.to_string_lossy().replace('\\', "/"),
        "via": via
    }))
}

fn mcp_tools_schema() -> Value {
    json!({
        "tools": [
            {
                "name": "s2a_status",
                "description": "Get Sketch2Asset status, active project, active API provider, all configured profiles & models, projects, and installed local ONNX DirectML GPU matting models.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "s2a_create_project",
                "description": "Create or update a Sketch2Asset project folder (with optional style rules) and optionally set it as the active project.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Project name (e.g. '横板游戏MCP测试')" },
                        "style": { "type": "string", "description": "Optional preset style tag (default 'default')" },
                        "style_prompt": { "type": "string", "description": "Optional project-wide visual style rules injected into all generations" },
                        "set_active": { "type": "boolean", "description": "Whether to switch activeProject to this project immediately (default true)" }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "s2a_switch_profile",
                "description": "Switch the active provider profile, active model (apiModel), or active project (activeProject), and persist to settings.json immediately.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "profile_id": { "type": "string", "description": "Optional provider profile ID or name to activate" },
                        "model": { "type": "string", "description": "Optional model ID to set as active default on the profile" },
                        "active_project": { "type": "string", "description": "Optional project name to set as activeProject" }
                    }
                }
            },
            {
                "name": "s2a_generate_asset",
                "description": "Generate a 2D game asset (item_icon, pixel_sprite, character_card, tile_texture, ui_element, free) with optional local GPU background removal, pixel-perfect integer grid quantization, and direct output to a game project path.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "prompt": { "type": "string", "description": "Detailed asset description" },
                        "archetype": { "type": "string", "enum": ["item_icon", "pixel_sprite", "character_card", "tile_texture", "ui_element", "free"] },
                        "size": { "type": "string", "description": "Canvas resolution e.g. 1024x1024, 1536x1024, 1024x1536, 1920x1080" },
                        "project": { "type": "string", "description": "Optional Sketch2Asset project folder name (overrides activeProject)" },
                        "profile_id": { "type": "string", "description": "Optional provider profile ID or name (overrides activeProfileId)" },
                        "model": { "type": "string", "description": "Optional model ID (overrides profile default model)" },
                        "style_prompt": { "type": "string", "description": "Optional additional style prompt" },
                        "negative_prompt": { "type": "string", "description": "Optional negative constraints" },
                        "sketch_path": { "type": "string", "description": "Optional reference/sketch image file path" },
                        "auto_remove_bg": { "type": "boolean", "description": "Automatically run local DirectML GPU matting + 3-stage anti-halo after generation (defaults to true for pixel_sprite)" },
                        "pixel_perfect": { "type": "boolean", "description": "Snap pixel art to a strict integer pixel grid with crisp alpha & zero mixels, plus export a 1x native pixel PNG (auto-enabled for pixel_sprite or pixel prompts)" },
                        "pixel_block_size": { "type": "integer", "description": "Optional explicit pixel block size (e.g. 4, 6, 8); auto-detected via edge-span histogram if omitted" },
                        "output_path": { "type": "string", "description": "Optional destination PNG file path inside the user's game repository" }
                    },
                    "required": ["prompt"]
                }
            },
            {
                "name": "s2a_remove_background",
                "description": "Run local DirectML GPU AI matting (BiRefNet / BEN2 / ISNet) on any local image file with foreground color unmixing, edge choke, and 4px color bleed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "input_path": { "type": "string", "description": "Absolute path to source image" },
                        "output_path": { "type": "string", "description": "Optional output PNG path (defaults to <input>_nobg.png)" },
                        "model_id": { "type": "string", "description": "birefnet | ben2 | birefnet-hrsod | birefnet-lite | isnet-general-use | isnet-anime" },
                        "choke": { "type": "integer", "description": "Alpha edge choke pixels (0..3, default 1)" }
                    },
                    "required": ["input_path"]
                }
            },
            {
                "name": "s2a_generate_character",
                "description": "Generate character turnaround sheets, identity-locked outfit reskins, animation sprite sheets (with automatic per-frame keyframe choreography, 8-connected island slicing, foot-baseline & horizontal pivot alignment, and pixel-perfect integer grid snapping), or expression grids.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "sub_mode": { "type": "string", "enum": ["turnaround", "outfit", "spritesheet", "expression"] },
                        "prompt": { "type": "string", "description": "Character description or animation action description" },
                        "rows": { "type": "integer", "description": "Optional grid rows for spritesheet (e.g. 2 for 2x3 or 1 for 1x6; auto-inferred from prompt if omitted)" },
                        "cols": { "type": "integer", "description": "Optional grid columns for spritesheet (e.g. 3 for 2x3 or 6 for 1x6; auto-inferred from prompt if omitted)" },
                        "project": { "type": "string", "description": "Optional Sketch2Asset project folder name (overrides activeProject)" },
                        "profile_id": { "type": "string", "description": "Optional provider profile ID or name (overrides activeProfileId)" },
                        "model": { "type": "string", "description": "Optional model ID (overrides profile default model)" },
                        "style_prompt": { "type": "string", "description": "Optional style instructions" },
                        "negative_prompt": { "type": "string", "description": "Optional negative constraints" },
                        "anchor_image_path": { "type": "string", "description": "Optional base character reference image path" },
                        "size": { "type": "string", "description": "Resolution e.g. 1536x1024 or 1024x1024" },
                        "auto_remove_bg": { "type": "boolean", "description": "Run local GPU matting automatically (defaults to true for spritesheet)" },
                        "pixel_perfect": { "type": "boolean", "description": "Quantize pixel-art sprite sheets onto a strict integer pixel grid (auto-enabled when prompt mentions pixel art)" },
                        "pixel_block_size": { "type": "integer", "description": "Optional explicit pixel block size (e.g. 4, 6, 8)" },
                        "export_frames": { "type": "boolean", "description": "Also export individual aligned frame PNGs into <stem>_frames/frame_01..N.png (default true for spritesheet)" },
                        "output_path": { "type": "string", "description": "Optional output PNG file path" }
                    },
                    "required": ["sub_mode", "prompt"]
                }
            },
            {
                "name": "s2a_edit_region",
                "description": "Modify a specific rectangular region [x, y, w, h] of an existing game asset image according to an instruction.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "input_path": { "type": "string", "description": "Path to original image" },
                        "x": { "type": "integer" },
                        "y": { "type": "integer" },
                        "w": { "type": "integer" },
                        "h": { "type": "integer" },
                        "instruction": { "type": "string", "description": "What to change in this region" },
                        "profile_id": { "type": "string", "description": "Optional provider profile ID or name" },
                        "model": { "type": "string", "description": "Optional model ID override" },
                        "output_path": { "type": "string", "description": "Optional output PNG path" }
                    },
                    "required": ["input_path", "instruction"]
                }
            },
            {
                "name": "s2a_list_assets",
                "description": "List recent generated game assets from the Sketch2Asset library.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "limit": { "type": "integer" }
                    }
                }
            }
        ]
    })
}

pub async fn run_mcp_stdio_loop() -> AppResult<()> {
    let state = init_standalone_state()?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line_res in stdin.lock().lines() {
        let Ok(line) = line_res else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };

        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

        // Notifications (no id) do not expect a response
        let Some(req_id) = id else {
            continue;
        };

        let response = match method {
            "initialize" => {
                let client_proto = params
                    .get("protocolVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05");
                json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "protocolVersion": client_proto,
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "sketch2asset",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                })
            }
            "ping" => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {}
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": mcp_tools_schema()
            }),
            "tools/call" => {
                state.reload_settings_from_disk();
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

                let res = match tool_name {
                    "s2a_status" => exec_status(&state).await,
                    "s2a_create_project" => exec_create_project(&state, &args).await,
                    "s2a_switch_profile" => exec_switch_profile(&state, &args).await,
                    "s2a_generate_asset" => exec_generate_asset(&state, &args).await,
                    "s2a_remove_background" => exec_remove_background(&state, &args).await,
                    "s2a_generate_character" => exec_generate_character(&state, &args).await,
                    "s2a_edit_region" => exec_edit_region(&state, &args).await,
                    "s2a_list_assets" => exec_list_assets(&state, &args).await,
                    _ => Err(AppError::new(
                        AppErrorCode::NotFound,
                        format!("Unknown tool: {}", tool_name),
                    )),
                };

                match res {
                    Ok(val) => json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&val).unwrap_or_default()
                            }],
                            "isError": false
                        }
                    }),
                    Err(err) => json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": format!("Error [{:?}]: {}", err.code, err.message)
                            }],
                            "isError": true
                        }
                    }),
                }
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            }),
        };

        if let Ok(serialized) = serde_json::to_string(&response) {
            let _ = writeln!(stdout, "{}", serialized);
            let _ = stdout.flush();
        }
    }

    Ok(())
}

pub async fn run_cli_args(args: &[String]) -> AppResult<()> {
    let state = init_standalone_state()?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");

    let mut map = serde_json::Map::new();
    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--remove-bg" => {
                map.insert("auto_remove_bg".to_string(), Value::Bool(true));
                i += 1;
            }
            "--prompt" | "-p" if i + 1 < args.len() => {
                map.insert("prompt".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--archetype" | "-a" if i + 1 < args.len() => {
                map.insert("archetype".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--size" | "-s" if i + 1 < args.len() => {
                map.insert("size".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--project" if i + 1 < args.len() => {
                map.insert("project".to_string(), Value::String(args[i + 1].clone()));
                map.insert("name".to_string(), Value::String(args[i + 1].clone()));
                map.insert(
                    "active_project".to_string(),
                    Value::String(args[i + 1].clone()),
                );
                i += 2;
            }
            "--profile" if i + 1 < args.len() => {
                map.insert("profile_id".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--style" if i + 1 < args.len() => {
                map.insert("style_prompt".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--output" | "-o" if i + 1 < args.len() => {
                map.insert("output_path".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--input" | "-i" if i + 1 < args.len() => {
                map.insert("input_path".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--model" | "-m" if i + 1 < args.len() => {
                map.insert("model_id".to_string(), Value::String(args[i + 1].clone()));
                map.insert("model".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--mode" if i + 1 < args.len() => {
                map.insert("sub_mode".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--anchor" if i + 1 < args.len() => {
                map.insert(
                    "anchor_image_path".to_string(),
                    Value::String(args[i + 1].clone()),
                );
                map.insert("sketch_path".to_string(), Value::String(args[i + 1].clone()));
                i += 2;
            }
            "--choke" if i + 1 < args.len() => {
                let c = args[i + 1].parse::<u64>().unwrap_or(1);
                map.insert("choke".to_string(), Value::Number(c.into()));
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    state.reload_settings_from_disk();
    let val_obj = Value::Object(map);
    let out = match sub {
        "status" => exec_status(&state).await?,
        "create-project" => exec_create_project(&state, &val_obj).await?,
        "switch-profile" => exec_switch_profile(&state, &val_obj).await?,
        "generate" => exec_generate_asset(&state, &val_obj).await?,
        "matting" => exec_remove_background(&state, &val_obj).await?,
        "character" => exec_generate_character(&state, &val_obj).await?,
        "list-assets" => exec_list_assets(&state, &val_obj).await?,
        _ => {
            return Err(AppError::new(
                AppErrorCode::InvalidInput,
                format!(
                    "Unknown CLI subcommand '{}'. Use: status | create-project | switch-profile | generate | matting | character | list-assets",
                    sub
                ),
            ))
        }
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&out).unwrap_or_default()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_mcp_background_prompt_rules() {
        let char_bg = resolve_mcp_background_prompt("character_card", true);
        assert!(char_bg.contains("#FFFFFF"));
        assert!(char_bg.contains("zero vignette"));
        assert!(char_bg.contains("zero radial or studio gradients"));

        let tile_bg = resolve_mcp_background_prompt("tile_texture", false);
        assert!(tile_bg.contains("seamless edge-to-edge"));

        let tile_cutout_bg = resolve_mcp_background_prompt("tile_texture", true);
        assert!(tile_cutout_bg.contains("#FFFFFF"));
    }

    #[test]
    fn test_build_character_generate_args_spritesheet_and_anchor() {
        let args = json!({
            "sub_mode": "spritesheet",
            "prompt": "knight sword slash 2x3",
            "anchor_image_path": "  C:/tmp/hero_anchor.png  "
        });
        let merged = build_character_generate_args(&args).unwrap();
        assert_eq!(merged["align_spritesheet"], Value::Bool(true));
        assert_eq!(merged["sprite_rows"], json!(2));
        assert_eq!(merged["sprite_cols"], json!(3));
        assert_eq!(merged["sketch_path"], json!("C:/tmp/hero_anchor.png"));
        assert_eq!(merged["size"], json!("1536x1024"));

        let blank_anchor_args = json!({
            "sub_mode": "turnaround",
            "prompt": "silver knight",
            "anchor_image_path": "   "
        });
        let merged_turnaround = build_character_generate_args(&blank_anchor_args).unwrap();
        assert!(merged_turnaround.get("sketch_path").is_none());
        let p = merged_turnaround["prompt"].as_str().unwrap();
        assert!(p.contains("pure flat #FFFFFF white background"));
        assert!(p.contains("zero vignette"));
    }
}
