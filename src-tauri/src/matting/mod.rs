use half::f16;
use image::{ imageops::FilterType, DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage };
use ort::{ ep, environment::Environment, session::Session, value::{ Tensor, TensorElementType, ValueType } };
use serde::{ Deserialize, Serialize };
use std::collections::VecDeque;
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
            // 1. 同级目录（绿色便携版 / 自包含安装目录）
            candidates.push(dir.join("onnxruntime.dll"));
            // 2. Tauri 打包资源目录（NSIS 安装包结构）
            candidates.push(dir.join("resources").join("onnxruntime.dll"));
            candidates.push(dir.join("bin").join("onnxruntime.dll"));
            candidates.push(dir.join("_up_").join("bin").join("onnxruntime.dll"));
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
    fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
    fn AddDllDirectory(new_directory: *const u16) -> isize;
    fn LoadLibraryExW(lp_lib_file_name: *const u16, h_file: isize, dw_flags: u32) -> isize;
    fn CreateMutexW(lp_mutex_attributes: *const std::ffi::c_void, b_initial_owner: i32, lp_name: *const u16) -> isize;
    fn WaitForSingleObject(h_handle: isize, dw_milliseconds: u32) -> u32;
    fn ReleaseMutex(h_mutex: isize) -> i32;
    fn CloseHandle(h_object: isize) -> i32;
}

/// Cross-process & multi-thread mutex guard that serializes local DirectML GPU matting inference
/// so concurrent MCP/CLI/GUI requests queue cleanly on the GPU instead of colliding in VRAM and falling back to CPU.
struct CrossProcessGpuLock {
    #[cfg(windows)]
    handle: isize,
}

impl CrossProcessGpuLock {
    fn acquire() -> Self {
        #[cfg(windows)]
        {
            let name = to_wide_null(std::ffi::OsStr::new("Local\\Sketch2Asset_DirectML_Inference_Mutex"));
            let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
            if handle != 0 {
                unsafe {
                    let _ = WaitForSingleObject(handle, 60_000);
                }
            }
            Self { handle }
        }
        #[cfg(not(windows))]
        {
            Self {}
        }
    }
}

impl Drop for CrossProcessGpuLock {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.handle != 0 {
            unsafe {
                let _ = ReleaseMutex(self.handle);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(windows)]
fn to_wide_null(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}

#[cfg(windows)]
fn setup_windows_dll_search_dir(dll_dir: &Path) {
    const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x00001000;
    let wide = to_wide_null(dll_dir.as_os_str());
    unsafe {
        // Ensure user-added DLL directories are searched even if WebView2/COM sets safe DLL search flags
        SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
        AddDllDirectory(wide.as_ptr());
        SetDllDirectoryW(wide.as_ptr());
    }

    let current_path = std::env::var("PATH").unwrap_or_default();
    let dir_str = dll_dir.to_string_lossy();
    if !current_path.contains(dir_str.as_ref()) {
        std::env::set_var("PATH", format!("{};{}", dir_str, current_path));
    }
}

#[cfg(windows)]
fn preload_dll_altered_path(dll_path: &Path) -> Result<(), String> {
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x00000008;
    let wide = to_wide_null(dll_path.as_os_str());
    let handle = unsafe { LoadLibraryExW(wide.as_ptr(), 0, LOAD_WITH_ALTERED_SEARCH_PATH) };
    if handle == 0 {
        let err = std::io::Error::last_os_error();
        Err(format!("LoadLibraryExW('{}') failed: {}", dll_path.display(), err))
    } else {
        Ok(())
    }
}

/// Call at the very beginning of `main()` before Tauri/WebView2 initializes so bundled
/// MSVC 14.50 runtime DLLs are mapped ahead of older system CRT DLLs in System32.
pub fn preload_startup_runtime_dlls() {
    #[cfg(windows)]
    {
        let mut dirs_to_check = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                dirs_to_check.push(dir.to_path_buf());
                dirs_to_check.push(dir.join("resources"));
                dirs_to_check.push(dir.join("bin"));
                dirs_to_check.push(dir.join("_up_").join("bin"));
                // Also support cargo/tauri dev runs inside target/<profile>
                if let Some(p3) = dir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
                    dirs_to_check.push(p3.join("bin"));
                }
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            dirs_to_check.push(cwd.join("bin"));
        }

        if let Some(dll_dir) = dirs_to_check
            .into_iter()
            .find(|d| d.join("onnxruntime.dll").exists() || d.join("vcruntime140.dll").exists())
        {
            setup_windows_dll_search_dir(&dll_dir);
            for name in [
                "vcruntime140.dll",
                "vcruntime140_1.dll",
                "msvcp140.dll",
                "msvcp140_1.dll",
                "msvcp140_2.dll",
            ] {
                let p = dll_dir.join(name);
                if p.exists() {
                    let _ = preload_dll_altered_path(&p);
                }
            }
        }
    }
}

fn ensure_ort_env(workspace_dir: &Path) -> AppResult<&'static Environment> {
    let res = ORT_ENV.get_or_init(|| {
        let dll_path = locate_onnxruntime_dll(workspace_dir)
            .ok_or_else(|| "onnxruntime.dll not found in bin/ or system paths".to_string())?;
        if let Some(dll_dir) = dll_path.parent() {
            #[cfg(windows)]
            {
                setup_windows_dll_search_dir(dll_dir);
                for dep in [
                    "vcruntime140.dll",
                    "vcruntime140_1.dll",
                    "msvcp140.dll",
                    "msvcp140_1.dll",
                    "msvcp140_2.dll",
                    "DirectML.dll",
                    "onnxruntime_providers_shared.dll",
                ] {
                    let p = dll_dir.join(dep);
                    if p.exists() {
                        let _ = preload_dll_altered_path(&p);
                    }
                }
                // Preload onnxruntime.dll itself with LOAD_WITH_ALTERED_SEARCH_PATH so
                // subsequent libloading::Library::new() reuses the already-mapped handle cleanly.
                preload_dll_altered_path(&dll_path)?;
            }

            #[cfg(not(windows))]
            {
                let current_path = std::env::var("PATH").unwrap_or_default();
                let dir_str = dll_dir.to_string_lossy();
                if !current_path.contains(dir_str.as_ref()) {
                    std::env::set_var("PATH", format!("{};{}", dir_str, current_path));
                }
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

    fn run_with_session<F>(
        &self,
        model_id: &str,
        device_pref: &str, // "auto" | "gpu" | "cpu"
        mut op: F,
    ) -> AppResult<(Vec<f32>, String)>
    where
        F: FnMut(&mut Session, &ModelPreprocConfig) -> AppResult<Vec<f32>>,
    {
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

        // Serialize GPU session creation and inference across all threads and OS processes
        let _gpu_lock = CrossProcessGpuLock::acquire();
        let mut guard = self.cached.lock().unwrap();
        let need_reload = match guard.as_ref() {
            Some(c) => {
                c.model_id != model_info.id
                    || c.device_pref != device_pref
                    || (device_pref != "cpu" && c.active_provider == "CPU")
            }
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
        let alpha_matte = match op(&mut cached_entry.session, &cfg) {
            Ok(m) => m,
            Err(e) if cached_entry.active_provider != "CPU" => {
                // If DirectML failed at runtime on a specific operator, gracefully fallback to CPU
                let cpu_sess = Session::builder(env)
                    .map_err(|err| AppError::new(AppErrorCode::MattingFailed, err.to_string()))?
                    .commit_from_file(&model_path)
                    .map_err(|err| AppError::new(AppErrorCode::MattingFailed, err.to_string()))?;
                cached_entry.session = cpu_sess;
                cached_entry.active_provider = "CPU".to_string();
                op(&mut cached_entry.session, &cfg).map_err(|_| e)?
            }
            Err(e) => return Err(e),
        };
        let final_ep = format!("{} · {}", model_info.name, cached_entry.active_provider);
        Ok((alpha_matte, final_ep))
    }

    pub fn run_matting(
        &self,
        img: &DynamicImage,
        model_id: &str,
        device_pref: &str, // "auto" | "gpu" | "cpu"
        enhanced: bool,
        choke: u8,
    ) -> AppResult<(RgbaImage, String)> {
        let (alpha_matte, final_ep) =
            self.run_with_session(model_id, device_pref, |session, cfg| {
                run_session_matte(session, cfg, img)
            })?;
        let rgba = compose_rgba(img, &alpha_matte, enhanced, choke.min(5));
        Ok((rgba, final_ep))
    }

    /// Runs high-resolution per-cell tiled neural matting on a `rows x cols` sprite sheet inside
    /// a single locked ONNX session, preserving fine hair strands and thin gaps before running `compose_rgba`.
    pub fn run_matting_spritesheet_grid(
        &self,
        img: &DynamicImage,
        rows: u32,
        cols: u32,
        model_id: &str,
        device_pref: &str,
        enhanced: bool,
        choke: u8,
    ) -> AppResult<(RgbaImage, String)> {
        let (w, h) = img.dimensions();
        if rows.saturating_mul(cols) <= 1 || w < cols.saturating_mul(8) || h < rows.saturating_mul(8)
        {
            return self.run_matting(img, model_id, device_pref, enhanced, choke);
        }

        let tile_rects = compute_grid_tile_rects(w, h, rows, cols);
        let (alpha_full, final_ep) =
            self.run_with_session(model_id, device_pref, |session, cfg| {
                let mut tile_mattes = Vec::with_capacity(tile_rects.len());
                for tile in &tile_rects {
                    let sub_img = img.crop_imm(tile.x, tile.y, tile.w, tile.h);
                    let matte = run_session_matte(session, cfg, &sub_img)?;
                    tile_mattes.push((*tile, matte));
                }
                Ok(stitch_tile_mattes(w, h, &tile_mattes))
            })?;

        let rgba = compose_rgba(img, &alpha_full, enhanced, choke.min(5));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridTile {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    cell_x0: u32,
    cell_y0: u32,
    cell_x1: u32,
    cell_y1: u32,
    has_left: bool,
    has_right: bool,
    has_top: bool,
    has_bottom: bool,
}

fn compute_grid_tile_rects(img_w: u32, img_h: u32, rows: u32, cols: u32) -> Vec<GridTile> {
    let rows = rows.max(1);
    let cols = cols.max(1);
    let mut tiles = Vec::with_capacity((rows * cols) as usize);
    for r in 0..rows {
        let cy0 = (r * img_h) / rows;
        let cy1 = ((r + 1) * img_h) / rows;
        let cell_h = cy1.saturating_sub(cy0).max(1);
        let pad_y = ((cell_h as f32) * 0.16)
            .round()
            .max(2.0)
            .min((cell_h / 2).max(1) as f32) as u32;

        for c in 0..cols {
            let cx0 = (c * img_w) / cols;
            let cx1 = ((c + 1) * img_w) / cols;
            let cell_w = cx1.saturating_sub(cx0).max(1);
            let pad_x = ((cell_w as f32) * 0.16)
                .round()
                .max(2.0)
                .min((cell_w / 2).max(1) as f32) as u32;

            let tx0 = cx0.saturating_sub(pad_x);
            let ty0 = cy0.saturating_sub(pad_y);
            let tx1 = (cx1 + pad_x).min(img_w);
            let ty1 = (cy1 + pad_y).min(img_h);

            tiles.push(GridTile {
                x: tx0,
                y: ty0,
                w: tx1.saturating_sub(tx0),
                h: ty1.saturating_sub(ty0),
                cell_x0: cx0,
                cell_y0: cy0,
                cell_x1: cx1,
                cell_y1: cy1,
                has_left: c > 0 && tx0 < cx0,
                has_right: c + 1 < cols && tx1 > cx1,
                has_top: r > 0 && ty0 < cy0,
                has_bottom: r + 1 < rows && ty1 > cy1,
            });
        }
    }
    tiles
}

#[inline]
fn smoothstep01(t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn stitch_tile_mattes(img_w: u32, img_h: u32, tiles: &[(GridTile, Vec<f32>)]) -> Vec<f32> {
    let w = img_w as usize;
    let h = img_h as usize;
    let n = w * h;
    let mut alpha_sum = vec![0.0_f32; n];
    let mut weight_sum = vec![0.0_f32; n];

    for (tile, matte) in tiles {
        let tw = tile.w as usize;
        let th = tile.h as usize;
        if tw == 0 || th == 0 || matte.len() < tw * th {
            continue;
        }
        let cell_w = (tile.cell_x1.saturating_sub(tile.cell_x0)).max(1) as f32;
        let cell_h = (tile.cell_y1.saturating_sub(tile.cell_y0)).max(1) as f32;
        let feather_l = ((tile.cell_x0.saturating_sub(tile.x)) as f32)
            .min(0.5 * cell_w)
            .max(1.0);
        let feather_r = (((tile.x + tile.w).saturating_sub(tile.cell_x1)) as f32)
            .min(0.5 * cell_w)
            .max(1.0);
        let feather_t = ((tile.cell_y0.saturating_sub(tile.y)) as f32)
            .min(0.5 * cell_h)
            .max(1.0);
        let feather_b = (((tile.y + tile.h).saturating_sub(tile.cell_y1)) as f32)
            .min(0.5 * cell_h)
            .max(1.0);

        for ly in 0..th {
            let gy = tile.y as usize + ly;
            if gy >= h {
                continue;
            }
            let yc = gy as f32 + 0.5;
            let wy_t = if tile.has_top {
                smoothstep01((yc - (tile.cell_y0 as f32 - feather_t)) / (2.0 * feather_t))
            } else {
                1.0
            };
            let wy_b = if tile.has_bottom {
                smoothstep01(((tile.cell_y1 as f32 + feather_b) - yc) / (2.0 * feather_b))
            } else {
                1.0
            };
            let wy = wy_t * wy_b;
            let dst_row = gy * w;
            let src_row = ly * tw;

            for lx in 0..tw {
                let gx = tile.x as usize + lx;
                if gx >= w {
                    continue;
                }
                let xc = gx as f32 + 0.5;
                let wx_l = if tile.has_left {
                    smoothstep01((xc - (tile.cell_x0 as f32 - feather_l)) / (2.0 * feather_l))
                } else {
                    1.0
                };
                let wx_r = if tile.has_right {
                    smoothstep01(((tile.cell_x1 as f32 + feather_r) - xc) / (2.0 * feather_r))
                } else {
                    1.0
                };
                let wt = (wx_l * wx_r * wy).max(1e-5);
                let dst_idx = dst_row + gx;
                alpha_sum[dst_idx] += matte[src_row + lx] * wt;
                weight_sum[dst_idx] += wt;
            }
        }
    }

    (0..n)
        .map(|i| {
            if weight_sum[i] > 0.0 {
                (alpha_sum[i] / weight_sum[i]).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect()
}

/// Exact 2-pass Rosenfeld-Pfaltz Manhattan (L1) distance transform in O(w * h).
fn manhattan_distance_map(mask: &[bool], w: usize, h: usize) -> Vec<u32> {
    let n = w * h;
    let inf = (w + h + 1) as u32;
    let mut dist = vec![inf; n];
    for i in 0..n {
        if mask[i] {
            dist[i] = 0;
        }
    }
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let idx = row + x;
            let mut d = dist[idx];
            if x > 0 {
                d = d.min(dist[idx - 1].saturating_add(1));
            }
            if y > 0 {
                d = d.min(dist[idx - w].saturating_add(1));
            }
            dist[idx] = d;
        }
    }
    for y in (0..h).rev() {
        let row = y * w;
        for x in (0..w).rev() {
            let idx = row + x;
            let mut d = dist[idx];
            if x + 1 < w {
                d = d.min(dist[idx + 1].saturating_add(1));
            }
            if y + 1 < h {
                d = d.min(dist[idx + w].saturating_add(1));
            }
            dist[idx] = d;
        }
    }
    dist
}

fn manhattan_distance_le(mask: &[bool], w: usize, h: usize, max_dist: u32) -> Vec<bool> {
    manhattan_distance_map(mask, w, h)
        .into_iter()
        .map(|d| d <= max_dist)
        .collect()
}

fn dilate_cross_small(mask: &[bool], w: usize, h: usize, iters: usize) -> Vec<bool> {
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

fn erode_cross(mask: &[bool], w: usize, h: usize, iters: usize) -> Vec<bool> {
    let mut cur = mask.to_vec();
    let mut next = cur.clone();
    for _ in 0..iters {
        next.copy_from_slice(&cur);
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let idx = row + x;
                if !cur[idx] {
                    continue;
                }
                let ok = (x == 0 || cur[idx - 1])
                    && (x + 1 == w || cur[idx + 1])
                    && (y == 0 || cur[idx - w])
                    && (y + 1 == h || cur[idx + w]);
                if !ok {
                    next[idx] = false;
                }
            }
        }
        std::mem::swap(&mut cur, &mut next);
    }
    cur
}

fn dilate_chebyshev(mask: &[bool], w: usize, h: usize, iters: usize) -> Vec<bool> {
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
                let mut hit = false;
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
                        if cur[(ny as usize) * w + (nx as usize)] {
                            hit = true;
                            break;
                        }
                    }
                    if hit {
                        break;
                    }
                }
                if hit {
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

/// Composes final RGBA cutout from raw image + neural alpha matte:
/// 1. Border / 4-corner median background color estimation (`bgc`).
/// 2. Dedicated `vfx_mask` detection (90px reachability + multi-hop chromatic stroke propagation) &
///    smooth foreground opacity rescue when `enhanced` is enabled.
/// 3. Backdrop Pocket & Fringe Cleanup (4-connected flood purge of trapped low-chroma cavities from
///    outer transparent / semi-transparent hair boundaries + soft-alpha fringe suppression, while
///    preserving isolated opaque interior white highlights).
/// 4. Protected `outer_halo` & `choke` (0..=5 px) that skip erosion on `vfx_mask` glow and thin `1-2px` tips.
/// 5. Foreground color unmixing + 4px color bleed into transparent area.
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
    if n == 0 {
        return RgbaImage::new(w_u32, h_u32);
    }
    let rgb_img = img.to_rgb8();
    let raw_rgb = rgb_img.as_raw();
    let mut alpha_src = vec![0.0_f32; n];
    let copy_len = n.min(alpha_in.len());
    alpha_src[..copy_len].copy_from_slice(&alpha_in[..copy_len]);

    // 1. Estimate background color (bgc) from border pixels where alpha < 0.1
    let mut br = Vec::new();
    let mut bg = Vec::new();
    let mut bb = Vec::new();

    let mut push_border = |x: usize, y: usize| {
        let idx = y * w + x;
        if alpha_src[idx] < 0.1 {
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

    let mut chroma_arr = vec![0.0_f32; n];
    let mut rel_chroma_arr = vec![0.0_f32; n];
    let mut d_pc_arr = vec![0.0_f32; n];
    let mut c_max_arr = vec![0.0_f32; n];
    for i in 0..n {
        let r = raw_rgb[i * 3] as f32;
        let g = raw_rgb[i * 3 + 1] as f32;
        let b = raw_rgb[i * 3 + 2] as f32;
        let c_max = r.max(g).max(b);
        let c_min = r.min(g).min(b);
        chroma_arr[i] = c_max - c_min;
        c_max_arr[i] = c_max;
        let dr = r - bgc[0];
        let dg = g - bgc[1];
        let db = b - bgc[2];
        rel_chroma_arr[i] = dr.max(dg).max(db) - dr.min(dg).min(db);
        d_pc_arr[i] = (dr * dr + dg * dg + db * db).sqrt();
    }

    let mut a2 = alpha_src.clone();

    // 2. Build dedicated VFX mask (90px reachability + multi-hop chromatic stroke propagation)
    let mut fg_seed: Vec<bool> = a2.iter().map(|&a| a > 0.7).collect();
    if !fg_seed.iter().any(|&v| v) {
        fg_seed = a2.iter().map(|&a| a > 0.2).collect();
    }
    let dist_fg = manhattan_distance_map(&fg_seed, w, h);

    let mut vfx_mask = vec![false; n];
    let mut vfx_queue = VecDeque::new();
    for i in 0..n {
        if dist_fg[i] <= 90 && rel_chroma_arr[i] > 22.0 {
            vfx_mask[i] = true;
            vfx_queue.push_back(i);
        }
    }
    while let Some(idx) = vfx_queue.pop_front() {
        let x = idx % w;
        let y = idx / w;
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
                if !vfx_mask[nidx] && rel_chroma_arr[nidx] > 18.0 {
                    vfx_mask[nidx] = true;
                    vfx_queue.push_back(nidx);
                }
            }
        }
    }

    if enhanced {
        for i in 0..n {
            if vfx_mask[i] {
                let rescue = ((rel_chroma_arr[i] - 16.0) / 36.0)
                    .max((d_pc_arr[i] - 32.0) / 70.0)
                    .clamp(0.0, 1.0);
                if rescue > a2[i] {
                    a2[i] = rescue;
                }
            } else if dist_fg[i] <= 45 && d_pc_arr[i] > 85.0 {
                let rescue = ((d_pc_arr[i] - 70.0) / 45.0).clamp(0.0, 1.0);
                if rescue > a2[i] {
                    a2[i] = rescue;
                }
            }
        }
    }

    // 3. Backdrop Pocket & Fringe Cleanup
    let bgc_max = bgc[0].max(bgc[1]).max(bgc[2]);
    let bgc_min = bgc[0].min(bgc[1]).min(bgc[2]);
    let bgc_chroma = bgc_max - bgc_min;
    let bgc_luma = 0.299 * bgc[0] + 0.587 * bgc[1] + 0.114 * bgc[2];
    let is_light_neutral_bg = bgc_luma >= 160.0 && bgc_chroma <= 40.0;
    let max_pocket_chroma = (bgc_chroma + 10.0).max(18.0);
    let max_pocket_chroma_strict = (bgc_chroma + 6.0).max(12.0);

    if is_light_neutral_bg {
        // 4-connected BFS avoids leaking diagonally across 1px dark outlines around white eyes/armor.
        let mut pocket_visited = vec![false; n];
        let mut pocket_queue = VecDeque::new();
        for i in 0..n {
            if vfx_mask[i] {
                continue;
            }
            let is_outer_transparent = alpha_src[i] < 0.12 && a2[i] < 0.15;
            let is_soft_hair_cavity = alpha_src[i] < 0.85
                && d_pc_arr[i] <= 30.0
                && chroma_arr[i] <= max_pocket_chroma
                && rel_chroma_arr[i] <= 15.0;
            if is_outer_transparent || is_soft_hair_cavity {
                pocket_visited[i] = true;
                pocket_queue.push_back(i);
                if is_soft_hair_cavity {
                    a2[i] = 0.0;
                }
            }
        }
        while let Some(idx) = pocket_queue.pop_front() {
            let x = idx % w;
            let y = idx / w;
            let neighbors = [
                if x > 0 { Some(idx - 1) } else { None },
                if x + 1 < w { Some(idx + 1) } else { None },
                if y > 0 { Some(idx - w) } else { None },
                if y + 1 < h { Some(idx + w) } else { None },
            ];
            for nidx in neighbors.into_iter().flatten() {
                if pocket_visited[nidx] || vfx_mask[nidx] {
                    continue;
                }
                let can_purge = if alpha_src[nidx] < 0.92 {
                    d_pc_arr[nidx] <= 30.0
                        && chroma_arr[nidx] <= max_pocket_chroma
                        && rel_chroma_arr[nidx] <= 15.0
                } else {
                    d_pc_arr[nidx] <= 24.0
                        && chroma_arr[nidx] <= max_pocket_chroma_strict
                        && rel_chroma_arr[nidx] <= 10.0
                };
                if can_purge {
                    pocket_visited[nidx] = true;
                    a2[nidx] = 0.0;
                    pocket_queue.push_back(nidx);
                }
            }
        }
    }

    // Soft-alpha & hair-edge halo suppression for residual low-chroma fringe near bgc
    let trans_mask: Vec<bool> = a2.iter().map(|&a| a < 0.08).collect();
    let near_trans = manhattan_distance_le(&trans_mask, w, h, 3);
    for i in 0..n {
        if vfx_mask[i] {
            continue;
        }
        let a = a2[i];
        if a <= 0.02 || a >= 0.92 {
            continue;
        }
        if chroma_arr[i] > max_pocket_chroma || rel_chroma_arr[i] > 15.0 {
            continue;
        }
        if near_trans[i] && d_pc_arr[i] < 54.0 {
            let cap = ((d_pc_arr[i] - 18.0) / 36.0).clamp(0.0, 1.0);
            a2[i] = a.min(cap);
        } else if a < 0.80 && d_pc_arr[i] < 44.0 {
            let cap = ((d_pc_arr[i] - 18.0) / 28.0).clamp(0.0, 1.0);
            a2[i] = a.min(cap);
        }
    }

    // 4. Protect thin 1-2px tips/strands & chromatic VFX glow from outer_halo attenuation and choke
    let choke_iters = choke.min(5) as usize;
    let thin_k = choke_iters.max(1);
    let fg_for_thin: Vec<bool> = (0..n)
        .map(|i| a2[i] >= 0.25 && (d_pc_arr[i] > 32.0 || rel_chroma_arr[i] > 22.0))
        .collect();
    let core_eroded = erode_cross(&fg_for_thin, w, h, thin_k);
    let thick_open = dilate_chebyshev(&core_eroded, w, h, thin_k);
    let thin_seed: Vec<bool> = (0..n)
        .map(|i| fg_for_thin[i] && !thick_open[i])
        .collect();
    let thin_dilated = dilate_cross_small(&thin_seed, w, h, thin_k);
    let thin_protect: Vec<bool> = (0..n)
        .map(|i| fg_for_thin[i] && thin_dilated[i])
        .collect();

    let vfx_protected: Vec<bool> = (0..n)
        .map(|i| {
            vfx_mask[i]
                && (alpha_src[i] < 0.92
                    || a2[i] < 0.92
                    || (c_max_arr[i] >= 170.0 && d_pc_arr[i] <= 185.0))
        })
        .collect();

    if enhanced {
        let low_alpha: Vec<bool> = a2.iter().map(|&a| a < 0.1).collect();
        let outer_halo = manhattan_distance_le(&low_alpha, w, h, 3);
        for i in 0..n {
            if outer_halo[i]
                && a2[i] > 0.02
                && !vfx_mask[i]
                && !thin_protect[i]
                && rel_chroma_arr[i] < 22.0
            {
                a2[i] = ((a2[i] - 0.15) / 0.85).clamp(0.0, 1.0);
            }
        }
    }

    // Edge Choke (0..=5 px 4-neighbor minimum erosion on unprotected thick boundaries)
    if choke_iters > 0 {
        let mut next_a = a2.clone();
        for _ in 0..choke_iters {
            next_a.copy_from_slice(&a2);
            for y in 0..h {
                let row = y * w;
                for x in 0..w {
                    let idx = row + x;
                    if thin_protect[idx] || vfx_protected[idx] {
                        continue;
                    }
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

    // 5. Foreground Color Unmixing on semi-transparent edge pixels (0.02 < a2 < 0.995)
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

    // 6. 4px Color Bleed into transparent area to prevent bilinear engine black/white fringes
    let mut solid: Vec<bool> = (0..n)
        .map(|i| a2[i] > 0.6 || (vfx_mask[i] && a2[i] > 0.08))
        .collect();
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

    #[test]
    fn test_hair_cavity_white_pocket_purged_with_fringe() {
        let w = 32_u32;
        let h = 32_u32;
        let mut rgb = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
        let mut alpha = vec![0.0_f32; (w * h) as usize];

        // Thick dark hair / limb ring enclosing pocket (11..21, 11..21)
        for y in 7..25 {
            for x in 7..25 {
                let in_ring = x < 11 || x >= 21 || y < 11 || y >= 21;
                if in_ring {
                    rgb.put_pixel(x, y, Rgb([36, 28, 24]));
                    alpha[(y * w + x) as usize] = 1.0;
                } else {
                    // Enclosed white backdrop pocket with semi-opaque / 1.0 residual alpha
                    rgb.put_pixel(x, y, Rgb([250, 250, 250]));
                    alpha[(y * w + x) as usize] = if x >= 14 && x < 18 && y >= 14 && y < 18 {
                        1.0
                    } else {
                        0.72
                    };
                }
            }
        }

        // Semi-transparent hair boundary fringe connecting outer edge to pocket
        for y in 7..11 {
            for x in 15..17 {
                rgb.put_pixel(x, y, Rgb([247, 247, 247]));
                alpha[(y * w + x) as usize] = 0.55;
            }
        }

        let dyn_img = DynamicImage::ImageRgb8(rgb);
        let res = compose_rgba(&dyn_img, &alpha, true, 1);

        // Enclosed hair/limb pocket (both 0.72 fringe and 1.0 center) must be cleanly purged
        assert_eq!(res.get_pixel(16, 16)[3], 0);
        assert_eq!(res.get_pixel(12, 16)[3], 0);
        assert_eq!(res.get_pixel(15, 9)[3], 0);
        // Solid hair body must stay opaque
        assert!(res.get_pixel(9, 16)[3] >= 240);
    }

    #[test]
    fn test_opaque_interior_white_armor_and_eye_highlight_preserved() {
        let w = 32_u32;
        let h = 32_u32;
        let mut rgb = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
        let mut alpha = vec![0.0_f32; (w * h) as usize];

        // Outer soft fringe around armor boundary
        for y in 5..27 {
            for x in 5..27 {
                rgb.put_pixel(x, y, Rgb([249, 249, 249]));
                alpha[(y * w + x) as usize] = 0.45;
            }
        }

        // Solid dark outline ring (thickness 2px) enclosing pure white armor/eye highlight
        for y in 7..25 {
            for x in 7..25 {
                let is_outline = x < 9 || x >= 23 || y < 9 || y >= 23;
                if is_outline {
                    rgb.put_pixel(x, y, Rgb([25, 26, 32]));
                } else {
                    rgb.put_pixel(x, y, Rgb([254, 254, 255]));
                }
                alpha[(y * w + x) as usize] = 1.0;
            }
        }

        // Also test a 1px diagonal diamond outline -> verify 4-connected BFS never leaks diagonally!
        let mut rgb_diag = RgbImage::from_pixel(17, 17, Rgb([255, 255, 255]));
        let mut alpha_diag = vec![0.0_f32; 17 * 17];
        for y in 0..17_i32 {
            for x in 0..17_i32 {
                let manh = (x - 8).abs() + (y - 8).abs();
                if manh == 4 {
                    rgb_diag.put_pixel(x as u32, y as u32, Rgb([20, 20, 25]));
                    alpha_diag[(y * 17 + x) as usize] = 1.0;
                } else if manh < 4 {
                    rgb_diag.put_pixel(x as u32, y as u32, Rgb([255, 255, 255]));
                    alpha_diag[(y * 17 + x) as usize] = 1.0;
                }
            }
        }

        let res = compose_rgba(&DynamicImage::ImageRgb8(rgb), &alpha, true, 1);
        assert!(res.get_pixel(16, 16)[3] >= 240);
        assert!(res.get_pixel(11, 11)[3] >= 240);
        assert_eq!(res.get_pixel(5, 16)[3], 0);

        let res_diag = compose_rgba(&DynamicImage::ImageRgb8(rgb_diag), &alpha_diag, true, 1);
        assert!(res_diag.get_pixel(8, 8)[3] >= 240);
        assert_eq!(res_diag.get_pixel(1, 1)[3], 0);
    }

    #[test]
    fn test_slash_vfx_glow_and_thin_blade_survive_enhanced_choke() {
        let w = 140_u32;
        let h = 64_u32;
        let mut rgb = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
        let mut alpha = vec![0.0_f32; (w * h) as usize];

        // Main opaque character/weapon core on left (x: 10..26, y: 16..48)
        for y in 16..48 {
            for x in 10..26 {
                rgb.put_pixel(x, y, Rgb([45, 55, 85]));
                alpha[(y * w + x) as usize] = 1.0;
            }
        }

        // Extended 2px cyan slash VFX line (chroma = 35, extending 102px from core -> > 90px multi-hop!)
        // Neural matte missed it (alpha_in = 0.0)
        for x in 26..128 {
            for y in 30..32 {
                rgb.put_pixel(x, y, Rgb([220, 245, 255]));
            }
            // Translucent outer aura (chroma = 23)
            rgb.put_pixel(x, 29, Rgb([232, 249, 255]));
            rgb.put_pixel(x, 32, Rgb([232, 249, 255]));
        }

        // 2px gold slash VFX arc (chroma = 37)
        for x in 26..88 {
            for y in 20..22 {
                rgb.put_pixel(x, y, Rgb([255, 246, 218]));
            }
        }

        // 2px neutral thin steel sword blade tip (chroma = 15 < 22, alpha_in = 1.0)
        for x in 26..62 {
            for y in 40..42 {
                rgb.put_pixel(x, y, Rgb([130, 135, 145]));
                alpha[(y * w + x) as usize] = 1.0;
            }
        }

        let res = compose_rgba(&DynamicImage::ImageRgb8(rgb), &alpha, true, 1);

        // Cyan slash at x=50 and far x=120 (>90px from core) must keep smooth translucent alpha
        let cyan_mid = res.get_pixel(50, 30)[3];
        let cyan_far = res.get_pixel(120, 31)[3];
        assert!(
            cyan_mid >= 90 && cyan_mid < 245,
            "expected smooth translucent cyan VFX alpha, got {}",
            cyan_mid
        );
        assert!(
            cyan_far >= 90 && cyan_far < 245,
            "expected multi-hop far cyan VFX (>90px) preserved, got {}",
            cyan_far
        );

        // Outer translucent aura must also survive softly without hard block or erasure
        let aura_a = res.get_pixel(60, 29)[3];
        assert!(
            aura_a >= 25 && aura_a < cyan_mid,
            "expected soft outer glow alpha < core, got {}",
            aura_a
        );

        // Gold slash at x=65 must survive
        let gold_a = res.get_pixel(65, 21)[3];
        assert!(
            gold_a >= 90 && gold_a < 245,
            "expected smooth gold VFX alpha, got {}",
            gold_a
        );

        // 2px thin steel blade tip must NOT be wiped out by choke=1
        assert!(
            res.get_pixel(58, 40)[3] >= 200,
            "expected 2px thin steel blade to survive choke=1"
        );
        // Attachment neck at x=26 must stay connected to core
        assert!(res.get_pixel(26, 40)[3] >= 200);

        // Pure white far background stays 0
        assert_eq!(res.get_pixel(125, 55)[3], 0);
    }

    #[test]
    fn test_grid_tile_rects_and_stitch_mattes_seamless() {
        let w = 100_u32;
        let h = 75_u32;
        let tiles = compute_grid_tile_rects(w, h, 2, 3);
        assert_eq!(tiles.len(), 6);
        for t in &tiles {
            assert!(t.x + t.w <= w);
            assert!(t.y + t.h <= h);
            assert!(t.w > 0 && t.h > 0);
        }

        // Build exact analytic gradient across each tile to verify partition-of-unity & seam continuity
        let analytic = |gx: u32, gy: u32| -> f32 {
            0.55 * (gx as f32 / w as f32) + 0.45 * (gy as f32 / h as f32)
        };
        let tile_mattes: Vec<(GridTile, Vec<f32>)> = tiles
            .iter()
            .map(|&t| {
                let mut m = vec![0.0_f32; (t.w * t.h) as usize];
                for ly in 0..t.h {
                    for lx in 0..t.w {
                        m[(ly * t.w + lx) as usize] = analytic(t.x + lx, t.y + ly);
                    }
                }
                (t, m)
            })
            .collect();

        let stitched = stitch_tile_mattes(w, h, &tile_mattes);
        assert_eq!(stitched.len(), (w * h) as usize);
        for gy in 0..h {
            for gx in 0..w {
                let got = stitched[(gy * w + gx) as usize];
                let exp = analytic(gx, gy);
                assert!(
                    (got - exp).abs() < 1e-4,
                    "stitch mismatch at ({}, {}): got {}, expected {}",
                    gx,
                    gy,
                    got,
                    exp
                );
            }
        }
    }

    #[test]
    fn test_preload_and_ort_env_load() {
        preload_startup_runtime_dlls();
        let ws = Path::new("..").canonicalize().unwrap_or_else(|_| PathBuf::from("D:/Sketch2Asset"));
        let env_res = ensure_ort_env(&ws);
        assert!(env_res.is_ok(), "ensure_ort_env failed: {:?}", env_res.err());
    }
}
