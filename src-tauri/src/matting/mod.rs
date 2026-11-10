use half::f16;
use image::{ imageops::FilterType, DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage };
use ort::{ ep, environment::Environment, session::Session, value::{ Tensor, TensorElementType, ValueType } };
use serde::{ Deserialize, Serialize };
use std::fs;
use std::path::{ Path, PathBuf };
use std::sync::{ Mutex, OnceLock };

use crate::error::{ AppError, AppErrorCode, AppResult };

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MattingModelInfo {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub description: String,
    pub size_mb: u32,
    pub installed: bool,
    pub file_path: Option<String>,
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivationKind {
    Sigmoid,
    MinMax,
    None,
}

#[derive(Debug, Clone)]
struct ModelPreprocConfig {
    input_size: u32,
    mean: [f32; 3],
    std: [f32; 3],
    div_max: bool,
    activation: ActivationKind,
}

fn model_preproc_config(file_name: &str) -> ModelPreprocConfig {
    let lower = file_name.to_lowercase();
    if lower.contains("rmbg-1") || lower.contains("rmbg_1") {
        ModelPreprocConfig {
            input_size: 1024,
            mean: [0.5, 0.5, 0.5],
            std: [1.0, 1.0, 1.0],
            div_max: false,
            activation: ActivationKind::MinMax,
        }
    } else if lower.contains("ben2") || lower.contains("ben_") {
        ModelPreprocConfig {
            input_size: 1024,
            mean: [0.485, 0.456, 0.406],
            std: [0.229, 0.224, 0.225],
            div_max: false,
            activation: ActivationKind::None,
        }
    } else if lower.contains("birefnet") || lower.contains("rmbg") {
        ModelPreprocConfig {
            input_size: 1024,
            mean: [0.485, 0.456, 0.406],
            std: [0.229, 0.224, 0.225],
            div_max: false,
            activation: ActivationKind::Sigmoid,
        }
    } else if lower.contains("u2net") {
        ModelPreprocConfig {
            input_size: 320,
            mean: [0.485, 0.456, 0.406],
            std: [0.229, 0.224, 0.225],
            div_max: true,
            activation: ActivationKind::MinMax,
        }
    } else {
        ModelPreprocConfig {
            input_size: 1024,
            mean: [0.5, 0.5, 0.5],
            std: [1.0, 1.0, 1.0],
            div_max: false,
            activation: ActivationKind::MinMax,
        }
    }
}

pub fn builtin_model_registry() -> Vec<MattingModelInfo> {
    vec![
        MattingModelInfo {
            id: "birefnet-lite".to_string(),
            name: "BiRefNet-Lite (推荐通用)".to_string(),
            file_name: "birefnet-lite.onnx".to_string(),
            description: "MIT 可商用，约 224MB，游戏道具/图标/角色通用，速度与精度均衡".to_string(),
            size_mb: 224,
            installed: false,
            file_path: None,
            urls: vec![
                "https://hf-mirror.com/onnx-community/BiRefNet_lite/resolve/main/onnx/model.onnx".to_string(),
                "https://huggingface.co/onnx-community/BiRefNet_lite/resolve/main/onnx/model.onnx".to_string(),
            ],
        },
        MattingModelInfo {
            id: "birefnet".to_string(),
            name: "BiRefNet Full FP16 (旗舰高精)".to_string(),
            file_name: "birefnet-full-fp16.onnx".to_string(),
            description: "家族最强通用版 FP16，约 489MB，复杂半透明边缘与发丝极佳".to_string(),
            size_mb: 489,
            installed: false,
            file_path: None,
            urls: vec![
                "https://hf-mirror.com/onnx-community/BiRefNet-ONNX/resolve/main/onnx/model_fp16.onnx".to_string(),
                "https://huggingface.co/onnx-community/BiRefNet-ONNX/resolve/main/onnx/model_fp16.onnx".to_string(),
            ],
        },
        MattingModelInfo {
            id: "ben2".to_string(),
            name: "BEN2 Base (新一代边缘精修)".to_string(),
            file_name: "ben2-base.onnx".to_string(),
            description: "2025 新一代高精前景分割模型，约 222MB，镂空与细微结构表现强".to_string(),
            size_mb: 222,
            installed: false,
            file_path: None,
            urls: vec![
                "https://hf-mirror.com/PramaLLC/BEN2/resolve/main/BEN2_Base.onnx".to_string(),
                "https://huggingface.co/PramaLLC/BEN2/resolve/main/BEN2_Base.onnx".to_string(),
            ],
        },
        MattingModelInfo {
            id: "birefnet-hrsod".to_string(),
            name: "BiRefNet-HRSOD FP16 (高清场景)".to_string(),
            file_name: "birefnet-hrsod-fp16.onnx".to_string(),
            description: "高分辨率显著目标特化训练版 FP16，约 489MB".to_string(),
            size_mb: 489,
            installed: false,
            file_path: None,
            urls: vec![
                "https://hf-mirror.com/onnx-community/BiRefNet-HRSOD_DHU-ONNX/resolve/main/onnx/model_fp16.onnx".to_string(),
                "https://huggingface.co/onnx-community/BiRefNet-HRSOD_DHU-ONNX/resolve/main/onnx/model_fp16.onnx".to_string(),
            ],
        },
        MattingModelInfo {
            id: "isnet-anime".to_string(),
            name: "ISNet-Anime (二次元角色特化)".to_string(),
            file_name: "isnetis.onnx".to_string(),
            description: "动漫立绘与二次元角色特化分割模型，约 176MB，速度快".to_string(),
            size_mb: 176,
            installed: false,
            file_path: None,
            urls: vec![
                "https://hf-mirror.com/skytnt/anime-seg/resolve/main/isnetis.onnx".to_string(),
                "https://huggingface.co/skytnt/anime-seg/resolve/main/isnetis.onnx".to_string(),
            ],
        },
        MattingModelInfo {
            id: "isnet-general-use".to_string(),
            name: "ISNet-General (经典轻量)".to_string(),
            file_name: "isnet-general-use.onnx".to_string(),
            description: "DIS/ISNet 通用物体分割，Apache-2.0，约 178MB，CPU 亦流畅".to_string(),
            size_mb: 178,
            installed: false,
            file_path: None,
            urls: vec![
                "https://hf-mirror.com/tomjackson2023/rembg/resolve/main/isnet-general-use.onnx".to_string(),
                "https://huggingface.co/tomjackson2023/rembg/resolve/main/isnet-general-use.onnx".to_string(),
                "https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx".to_string(),
            ],
        },
    ]
}

static ORT_ENV: OnceLock<Result<Environment, String>> = OnceLock::new();

fn locate_onnxruntime_dll(workspace_dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // 1. 同级目录（绿色便携版 / 自包含发布目录）
            candidates.push(dir.join("onnxruntime.dll"));
            // 2. Tauri 打包资源目录（NSIS 安装包默认结构）
            candidates.push(dir.join("resources").join("onnxruntime.dll"));
            candidates.push(dir.join("bin").join("onnxruntime.dll"));
        }
    }
    // 3. 项目工作空间目录与 bin 目录
    candidates.push(workspace_dir.join("bin").join("onnxruntime.dll"));
    candidates.push(workspace_dir.join("resources").join("onnxruntime.dll"));
    candidates.push(workspace_dir.join("onnxruntime.dll"));
    if let Some(local_app_data) = dirs::data_local_dir() {
        candidates.push(
            local_app_data
                .join("Sketch2Asset")
                .join("bin")
                .join("onnxruntime.dll"),
        );
    }
    candidates.into_iter().find(|p| p.exists())
}

#[cfg(windows)]
extern "system" {
    fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
}

fn ensure_ort_env(workspace_dir: &Path) -> AppResult<&'static Environment> {
    let res = ORT_ENV.get_or_init(|| {
        let dll_path = locate_onnxruntime_dll(workspace_dir)
            .ok_or_else(|| "onnxruntime.dll not found in bin/ or system paths".to_string())?;
        if let Some(dll_dir) = dll_path.parent() {
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                let wide: Vec<u16> = dll_dir.as_os_str().encode_wide().chain(Some(0)).collect();
                unsafe {
                    SetDllDirectoryW(wide.as_ptr());
                }
            }

            let current_path = std::env::var("PATH").unwrap_or_default();
            let dir_str = dll_dir.to_string_lossy();
            if !current_path.contains(dir_str.as_ref()) {
                std::env::set_var("PATH", format!("{};{}", dir_str, current_path));
            }
        }
        let builder = ort::init_from(&dll_path).map_err(|e| e.to_string())?;
        builder.build().map_err(|e| e.to_string())
    });

    match res {
        Ok(env) => Ok(env),
        Err(msg) => Err(AppError::new(AppErrorCode::MattingFailed, msg.clone())),
    }
}

struct CachedMattingSession {
    model_id: String,
    device_pref: String,
    active_provider: String,
    session: Session,
}

pub struct MattingEngine {
    pub workspace_models_dir: PathBuf,
    pub appdata_models_dir: PathBuf,
    pub workspace_dir: PathBuf,
    cached: Mutex<Option<CachedMattingSession>>,
}

impl MattingEngine {
    pub fn new(workspace_dir: PathBuf, app_data_dir: &Path) -> Self {
        let workspace_models_dir = workspace_dir.join("models");
        let appdata_models_dir = app_data_dir.join("models");
        let _ = fs::create_dir_all(&appdata_models_dir);
        Self {
            workspace_models_dir,
            appdata_models_dir,
            workspace_dir,
            cached: Mutex::new(None),
        }
    }

    pub fn resolve_model_path(&self, file_name: &str) -> Option<PathBuf> {
        let p1 = self.appdata_models_dir.join(file_name);
        if p1.exists() && p1.metadata().map(|m| m.len() > 1024 * 1024).unwrap_or(false) {
            return Some(p1);
        }
        let p2 = self.workspace_models_dir.join(file_name);
        if p2.exists() && p2.metadata().map(|m| m.len() > 1024 * 1024).unwrap_or(false) {
            return Some(p2);
        }
        None
    }

    pub fn list_models(&self) -> Vec<MattingModelInfo> {
        let mut list = builtin_model_registry();
        for item in &mut list {
            if let Some(p) = self.resolve_model_path(&item.file_name) {
                item.installed = true;
                item.file_path = Some(p.to_string_lossy().replace('\\', "/"));
            }
        }
        list
    }

    pub fn delete_model(&self, model_id: &str) -> AppResult<()> {
        let models = builtin_model_registry();
        let target = models
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| AppError::new(AppErrorCode::NotFound, "Unknown matting model ID"))?;

        {
            let mut guard = self.cached.lock().unwrap();
            if guard.as_ref().map(|c| c.model_id == model_id).unwrap_or(false) {
                *guard = None;
            }
        }

        let p1 = self.workspace_models_dir.join(&target.file_name);
        if p1.exists() {
            fs::remove_file(&p1)?;
        }
        let p2 = self.appdata_models_dir.join(&target.file_name);
        if p2.exists() {
            fs::remove_file(&p2)?;
        }
        Ok(())
    }

    pub fn run_matting(
        &self,
        img: &DynamicImage,
        model_id: &str,
        device_pref: &str, // "auto" | "gpu" | "cpu"
        enhanced: bool,
        choke: u8,
    ) -> AppResult<(RgbaImage, String)> {
        let models = self.list_models();
        let model_info = models
            .iter()
            .find(|m| m.id == model_id)
            .or_else(|| models.iter().find(|m| m.installed))
            .ok_or_else(|| {
                AppError::new(
                    AppErrorCode::ModelMissing,
                    format!("Matting model '{}' is not installed", model_id),
                )
            })?;

        let model_path = self.resolve_model_path(&model_info.file_name).ok_or_else(|| {
            AppError::new(
                AppErrorCode::ModelMissing,
                format!(
                    "Model file '{}' not found. Please download it in the Studio panel first.",
                    model_info.file_name
                ),
            )
        })?;

        let env = ensure_ort_env(&self.workspace_dir)?;
        let cfg = model_preproc_config(&model_info.file_name);

        let mut guard = self.cached.lock().unwrap();
        let need_reload = match guard.as_ref() {
            Some(c) => c.model_id != model_info.id || c.device_pref != device_pref,
            None => true,
        };

        if need_reload {
            let mut loaded_session: Option<(Session, String)> = None;
            if device_pref != "cpu" {
                if let Ok(builder) = Session::builder(env) {
                    if let Ok(mut b_dml) = builder.with_execution_providers([ep::DirectML::default().build()]) {
                        if let Ok(sess) = b_dml.commit_from_file(&model_path) {
                            loaded_session = Some((sess, "DirectML (GPU)".to_string()));
                        }
                    }
                }
            }
            if loaded_session.is_none() {
                let sess = Session::builder(env)
                    .map_err(|e| AppError::new(AppErrorCode::MattingFailed, e.to_string()))?
                    .commit_from_file(&model_path)
                    .map_err(|e| AppError::new(AppErrorCode::MattingFailed, e.to_string()))?;
                loaded_session = Some((sess, "CPU".to_string()));
            }
            let (session, active_provider) = loaded_session.unwrap();
            *guard = Some(CachedMattingSession {
                model_id: model_info.id.clone(),
                device_pref: device_pref.to_string(),
                active_provider,
                session,
            });
        }

        let cached_entry = guard.as_mut().unwrap();
        let active_ep = cached_entry.active_provider.clone();
        let alpha_matte = match run_session_matte(&mut cached_entry.session, &cfg, img) {
            Ok(m) => m,
            Err(e) if cached_entry.active_provider != "CPU" => {
                // If DirectML failed at runtime on a specific operator, gracefully fallback to CPU
                let cpu_sess = Session::builder(env)
                    .map_err(|err| AppError::new(AppErrorCode::MattingFailed, err.to_string()))?
                    .commit_from_file(&model_path)
                    .map_err(|err| AppError::new(AppErrorCode::MattingFailed, err.to_string()))?;
                cached_entry.session = cpu_sess;
                cached_entry.active_provider = "CPU".to_string();
                run_session_matte(&mut cached_entry.session, &cfg, img).map_err(|_| e)?
            }
            Err(e) => return Err(e),
        };
        let final_ep = format!("{} · {}", model_info.name, active_ep);
        drop(guard);

        let rgba = compose_rgba(img, &alpha_matte, enhanced, choke.min(5));
        Ok((rgba, final_ep))
    }
}

fn run_session_matte(
    session: &mut Session,
    cfg: &ModelPreprocConfig,
    img: &DynamicImage,
) -> AppResult<Vec<f32>> {
    let (ow, oh) = img.dimensions();
    let s = cfg.input_size;
    let resized = img.resize_exact(s, s, FilterType::Lanczos3).to_rgb8();
    let raw = resized.as_raw();

    let denom = if cfg.div_max {
        let max_v = raw.iter().copied().max().unwrap_or(255) as f32;
        max_v.max(1e-6)
    } else {
        255.0
    };

    let plane = (s * s) as usize;
    let mut chw = vec![0.0_f32; 3 * plane];
    for i in 0..plane {
        let r = (raw[i * 3] as f32) / denom;
        let g = (raw[i * 3 + 1] as f32) / denom;
        let b = (raw[i * 3 + 2] as f32) / denom;
        chw[i] = (r - cfg.mean[0]) / cfg.std[0];
        chw[plane + i] = (g - cfg.mean[1]) / cfg.std[1];
        chw[2 * plane + i] = (b - cfg.mean[2]) / cfg.std[2];
    }

    let expects_f16 = session
        .inputs()
        .first()
        .map(|outlet| {
            matches!(
                outlet.dtype(),
                ValueType::Tensor {
                    ty: TensorElementType::Float16,
                    ..
                }
            )
        })
        .unwrap_or(false);

    let shape = [1_usize, 3, s as usize, s as usize];
    let outputs = if expects_f16 {
        let chw_f16: Vec<f16> = chw.iter().map(|&v| f16::from_f32(v)).collect();
        let input_tensor = Tensor::from_array((shape, chw_f16.into_boxed_slice()))
            .map_err(|e| AppError::new(AppErrorCode::MattingFailed, e.to_string()))?;
        session
            .run(ort::inputs![input_tensor])
            .map_err(|e| AppError::new(AppErrorCode::MattingFailed, e.to_string()))?
    } else {
        let input_tensor = Tensor::from_array((shape, chw.into_boxed_slice()))
            .map_err(|e| AppError::new(AppErrorCode::MattingFailed, e.to_string()))?;
        session
            .run(ort::inputs![input_tensor])
            .map_err(|e| AppError::new(AppErrorCode::MattingFailed, e.to_string()))?
    };

    let out_val = &outputs[0];
    let mut raw_matte: Vec<f32> = if let Ok((_, slice_f32)) = out_val.try_extract_tensor::<f32>() {
        slice_f32.iter().take(plane).copied().collect()
    } else if let Ok((_, slice_f16)) = out_val.try_extract_tensor::<f16>() {
        slice_f16.iter().take(plane).map(|v| v.to_f32()).collect()
    } else {
        return Err(AppError::new(
            AppErrorCode::MattingFailed,
            "Unsupported ONNX output tensor element type",
        ));
    };

    match cfg.activation {
        ActivationKind::Sigmoid => {
            for v in &mut raw_matte {
                let c = v.clamp(-30.0, 30.0);
                *v = 1.0 / (1.0 + (-c).exp());
            }
        }
        ActivationKind::None => {
            for v in &mut raw_matte {
                *v = v.clamp(0.0, 1.0);
            }
        }
        ActivationKind::MinMax => {
            let mut min_v = f32::INFINITY;
            let mut max_v = f32::NEG_INFINITY;
            for &v in &raw_matte {
                if v < min_v {
                    min_v = v;
                }
                if v > max_v {
                    max_v = v;
                }
            }
            let span = (max_v - min_v).max(1e-6);
            for v in &mut raw_matte {
                *v = (*v - min_v) / span;
            }
        }
    }

    // Resize matte back to (ow, oh) using Lanczos3
    let matte_u8: Vec<u8> = raw_matte
        .iter()
        .map(|&v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect();
    let gray_img: ImageBuffer<image::Luma<u8>, Vec<u8>> =
        ImageBuffer::from_raw(s, s, matte_u8).ok_or_else(|| {
            AppError::new(AppErrorCode::MattingFailed, "Failed to construct matte buffer")
        })?;
    let resized_gray = if ow == s && oh == s {
        gray_img
    } else {
        image::imageops::resize(&gray_img, ow, oh, FilterType::Lanczos3)
    };

    let final_matte: Vec<f32> = resized_gray
        .as_raw()
        .iter()
        .map(|&b| {
            let a = (b as f32) / 255.0;
            ((a - 0.03) / 0.94).clamp(0.0, 1.0)
        })
        .collect();

    Ok(final_matte)
}

fn dilate_cross(mask: &[bool], w: usize, h: usize, iters: usize) -> Vec<bool> {
    let mut cur = mask.to_vec();
    let mut next = cur.clone();
    for _ in 0..iters {
        next.copy_from_slice(&cur);
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let idx = row + x;
                if cur[idx] {
                    continue;
                }
                if (x > 0 && cur[idx - 1])
                    || (x + 1 < w && cur[idx + 1])
                    || (y > 0 && cur[idx - w])
                    || (y + 1 < h && cur[idx + w])
                {
                    next[idx] = true;
                }
            }
        }
        std::mem::swap(&mut cur, &mut next);
    }
    cur
}

fn median_f32(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 255.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        0.5 * (values[mid - 1] + values[mid])
    }
}

/// Ported 1:1 from `matting_worker.py::compose_rgba`:
/// 1. Border / 4-corner median background color estimation (`bgc`).
/// 2. Optional `enhanced` recovery of high-chroma details near foreground + outer halo tightening.
/// 3. `choke` (0..=5 px) 4-neighbor minimum alpha erosion.
/// 4. Foreground color unmixing (`(rgb - (1 - a) * bgc) / max(a, 0.08)`).
/// 5. 4px 3x3 weighted color bleed into transparent pixels to prevent game-engine bilinear fringes.
pub fn compose_rgba(
    img: &DynamicImage,
    alpha_in: &[f32],
    enhanced: bool,
    choke: u8,
) -> RgbaImage {
    let (w_u32, h_u32) = img.dimensions();
    let w = w_u32 as usize;
    let h = h_u32 as usize;
    let n = w * h;
    let rgb_img = img.to_rgb8();
    let raw_rgb = rgb_img.as_raw();

    // 1. Estimate background color (bgc) from border pixels where alpha < 0.1
    let mut br = Vec::new();
    let mut bg = Vec::new();
    let mut bb = Vec::new();

    let mut push_border = |x: usize, y: usize| {
        let idx = y * w + x;
        if alpha_in[idx] < 0.1 {
            br.push(raw_rgb[idx * 3] as f32);
            bg.push(raw_rgb[idx * 3 + 1] as f32);
            bb.push(raw_rgb[idx * 3 + 2] as f32);
        }
    };

    for x in 0..w {
        push_border(x, 0);
        if h > 1 {
            push_border(x, h - 1);
        }
    }
    for y in 0..h {
        push_border(0, y);
        if w > 1 {
            push_border(w - 1, y);
        }
    }

    let bgc = if br.len() > 50 {
        [median_f32(&mut br), median_f32(&mut bg), median_f32(&mut bb)]
    } else {
        let corners = [
            0,
            w.saturating_sub(1),
            (h.saturating_sub(1)) * w,
            (h.saturating_sub(1)) * w + w.saturating_sub(1),
        ];
        let mut cr: Vec<f32> = corners.iter().map(|&i| raw_rgb[i * 3] as f32).collect();
        let mut cg: Vec<f32> = corners.iter().map(|&i| raw_rgb[i * 3 + 1] as f32).collect();
        let mut cb: Vec<f32> = corners.iter().map(|&i| raw_rgb[i * 3 + 2] as f32).collect();
        [median_f32(&mut cr), median_f32(&mut cg), median_f32(&mut cb)]
    };

    let mut a2 = alpha_in.to_vec();

    // 2. Optional Enhanced Recovery pass
    if enhanced {
        let fg_seed: Vec<bool> = a2.iter().map(|&a| a > 0.7).collect();
        let near_fg = dilate_cross(&fg_seed, w, h, 45);
        for i in 0..n {
            if !near_fg[i] {
                continue;
            }
            let r = raw_rgb[i * 3] as f32;
            let g = raw_rgb[i * 3 + 1] as f32;
            let b = raw_rgb[i * 3 + 2] as f32;
            let c_max = r.max(g).max(b);
            let c_min = r.min(g).min(b);
            let chroma = c_max - c_min;
            let dr = r - bgc[0];
            let dg = g - bgc[1];
            let db = b - bgc[2];
            let d_pc = (dr * dr + dg * dg + db * db).sqrt();
            if chroma > 40.0 || d_pc > 100.0 {
                let rescue = ((chroma - 25.0) / 30.0).max((d_pc - 70.0) / 45.0).clamp(0.0, 1.0);
                if rescue > a2[i] {
                    a2[i] = rescue;
                }
            }
        }

        let low_alpha: Vec<bool> = a2.iter().map(|&a| a < 0.1).collect();
        let outer_halo = dilate_cross(&low_alpha, w, h, 3);
        for i in 0..n {
            if outer_halo[i] && a2[i] > 0.02 {
                a2[i] = ((a2[i] - 0.15) / 0.85).clamp(0.0, 1.0);
            }
        }
    }

    // 3. Edge Choke (0..=5 px 4-neighbor minimum erosion)
    let choke_iters = choke.min(5) as usize;
    if choke_iters > 0 {
        let mut next_a = a2.clone();
        for _ in 0..choke_iters {
            next_a.copy_from_slice(&a2);
            for y in 0..h {
                let row = y * w;
                for x in 0..w {
                    let idx = row + x;
                    let mut m = a2[idx];
                    if x > 0 && a2[idx - 1] < m {
                        m = a2[idx - 1];
                    }
                    if x + 1 < w && a2[idx + 1] < m {
                        m = a2[idx + 1];
                    }
                    if y > 0 && a2[idx - w] < m {
                        m = a2[idx - w];
                    }
                    if y + 1 < h && a2[idx + w] < m {
                        m = a2[idx + w];
                    }
                    next_a[idx] = m;
                }
            }
            std::mem::swap(&mut a2, &mut next_a);
        }
    }

    // 4. Foreground Color Unmixing on semi-transparent edge pixels (0.02 < a2 < 0.995)
    let mut rgb_f32 = vec![0.0_f32; n * 3];
    for i in 0..n {
        let r = raw_rgb[i * 3] as f32;
        let g = raw_rgb[i * 3 + 1] as f32;
        let b = raw_rgb[i * 3 + 2] as f32;
        let a = a2[i];
        if a > 0.02 && a < 0.995 {
            let safe_a = a.max(0.08);
            let ur = ((r - (1.0 - a) * bgc[0]) / safe_a).clamp(0.0, 255.0);
            let ug = ((g - (1.0 - a) * bgc[1]) / safe_a).clamp(0.0, 255.0);
            let ub = ((b - (1.0 - a) * bgc[2]) / safe_a).clamp(0.0, 255.0);
            let fade = (a / 0.15).clamp(0.0, 1.0);
            rgb_f32[i * 3] = ur * fade + r * (1.0 - fade);
            rgb_f32[i * 3 + 1] = ug * fade + g * (1.0 - fade);
            rgb_f32[i * 3 + 2] = ub * fade + b * (1.0 - fade);
        } else {
            rgb_f32[i * 3] = r;
            rgb_f32[i * 3 + 1] = g;
            rgb_f32[i * 3 + 2] = b;
        }
    }

    // 5. 4px Color Bleed into transparent area to prevent bilinear engine black/white fringes
    let mut solid: Vec<bool> = a2.iter().map(|&a| a > 0.6).collect();
    let mut next_rgb = rgb_f32.clone();
    let mut next_solid = solid.clone();

    for _ in 0..4 {
        next_rgb.copy_from_slice(&rgb_f32);
        next_solid.copy_from_slice(&solid);
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let idx = row + x;
                if solid[idx] {
                    continue;
                }
                let mut sr = 0.0_f32;
                let mut sg = 0.0_f32;
                let mut sb = 0.0_f32;
                let mut wt = 0.0_f32;

                for dy in -1_isize..=1 {
                    let ny = y as isize + dy;
                    if ny < 0 || ny >= h as isize {
                        continue;
                    }
                    for dx in -1_isize..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = x as isize + dx;
                        if nx < 0 || nx >= w as isize {
                            continue;
                        }
                        let nidx = (ny as usize) * w + (nx as usize);
                        if solid[nidx] {
                            let w_nb = if dx == 0 || dy == 0 { 1.0 } else { 0.7 };
                            sr += rgb_f32[nidx * 3] * w_nb;
                            sg += rgb_f32[nidx * 3 + 1] * w_nb;
                            sb += rgb_f32[nidx * 3 + 2] * w_nb;
                            wt += w_nb;
                        }
                    }
                }

                if wt > 0.0 {
                    next_rgb[idx * 3] = sr / wt;
                    next_rgb[idx * 3 + 1] = sg / wt;
                    next_rgb[idx * 3 + 2] = sb / wt;
                    next_solid[idx] = true;
                }
            }
        }
        std::mem::swap(&mut rgb_f32, &mut next_rgb);
        std::mem::swap(&mut solid, &mut next_solid);
    }

    let mut out_img = RgbaImage::new(w_u32, h_u32);
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let r = rgb_f32[idx * 3].round().clamp(0.0, 255.0) as u8;
            let g = rgb_f32[idx * 3 + 1].round().clamp(0.0, 255.0) as u8;
            let b = rgb_f32[idx * 3 + 2].round().clamp(0.0, 255.0) as u8;
            let a = (a2[idx] * 255.0).round().clamp(0.0, 255.0) as u8;
            out_img.put_pixel(x as u32, y as u32, Rgba([r, g, b, a]));
        }
    }
    out_img
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ DynamicImage, Rgb, RgbImage };

    #[test]
    fn test_compose_rgba_unmix_choke_and_bleed() {
        let mut rgb = RgbImage::from_pixel(16, 16, Rgb([255, 255, 255]));
        for y in 4..12 {
            for x in 4..12 {
                rgb.put_pixel(x, y, Rgb([220, 40, 40]));
            }
        }
        let dyn_img = DynamicImage::ImageRgb8(rgb);
        let mut alpha = vec![0.0_f32; 16 * 16];
        for y in 4..12 {
            for x in 4..12 {
                alpha[y * 16 + x] = 1.0;
            }
        }
        let res = compose_rgba(&dyn_img, &alpha, true, 1);
        assert_eq!(res.dimensions(), (16, 16));
        // Center pixel should remain opaque red
        let center = res.get_pixel(8, 8);
        assert_eq!(center[3], 255);
        assert!(center[0] > 200 && center[1] < 60);
        // Choke=1 should erode the outer border (4, 8) to 0 alpha, but 4px color bleed should fill its RGB with red!
        let eroded_edge = res.get_pixel(4, 8);
        assert_eq!(eroded_edge[3], 0);
        assert!(eroded_edge[0] > 180);
    }
}
