use half::f16;
use image::{ imageops::FilterType, DynamicImage, GenericImageView, Rgba, RgbaImage };
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

    /// Explicitly drop the cached ONNX DirectML/CPU session to immediately reclaim GPU VRAM
    /// before spawning VRAM-intensive sidecars such as See-Through Layer Decomposition.
    pub fn unload_session(&self) {
        let _gpu_lock = CrossProcessGpuLock::acquire();
        if let Ok(mut guard) = self.cached.lock() {
            *guard = None;
        }
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

    // Upsample matte directly in f32 via monotone bilinear interpolation (avoids u8 quantization & Lanczos3 ringing halos)
    let final_matte = if ow == s && oh == s {
        raw_matte
            .into_iter()
            .map(|a| {
                if a < 0.006 {
                    0.0
                } else if a > 0.998 {
                    1.0
                } else {
                    a
                }
            })
            .collect()
    } else {
        resize_f32_bilinear(&raw_matte, s as usize, s as usize, ow as usize, oh as usize)
            .into_iter()
            .map(|a| {
                if a < 0.006 {
                    0.0
                } else if a > 0.998 {
                    1.0
                } else {
                    a
                }
            })
            .collect()
    };

    Ok(final_matte)
}

fn resize_f32_bilinear(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
    if dw == 0 || dh == 0 {
        return Vec::new();
    }
    if sw == 0 || sh == 0 || src.len() < sw * sh {
        return vec![0.0; dw * dh];
    }
    let mut dst = vec![0.0_f32; dw * dh];
    let scale_x = (sw as f32) / (dw as f32);
    let scale_y = (sh as f32) / (dh as f32);

    for dy in 0..dh {
        let fy = ((dy as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (sh - 1) as f32);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let wy = fy - (y0 as f32);
        let row0 = y0 * sw;
        let row1 = y1 * sw;
        let dst_row = dy * dw;

        for dx in 0..dw {
            let fx = ((dx as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (sw - 1) as f32);
            let x0 = fx.floor() as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let wx = fx - (x0 as f32);

            let v00 = src[row0 + x0];
            let v10 = src[row0 + x1];
            let v01 = src[row1 + x0];
            let v11 = src[row1 + x1];

            let top = v00 + (v10 - v00) * wx;
            let bot = v01 + (v11 - v01) * wx;
            dst[dst_row + dx] = (top + (bot - top) * wy).clamp(0.0, 1.0);
        }
    }
    dst
}

/// O(w * h) separable integral box-mean filter for Guided Image Filtering.
fn box_mean_2d(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    if r == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let mut tmp = vec![0.0_f32; w * h];
    let mut dst = vec![0.0_f32; w * h];

    // Horizontal pass with running prefix sum
    let mut row_prefix = vec![0.0_f64; w + 1];
    for y in 0..h {
        let off = y * w;
        row_prefix[0] = 0.0;
        for x in 0..w {
            row_prefix[x + 1] = row_prefix[x] + (src[off + x] as f64);
        }
        for x in 0..w {
            let x0 = x.saturating_sub(r);
            let x1 = (x + r).min(w - 1);
            let cnt = (x1 - x0 + 1) as f64;
            tmp[off + x] = ((row_prefix[x1 + 1] - row_prefix[x0]) / cnt) as f32;
        }
    }

    // Vertical pass with column prefix sum
    let mut col_prefix = vec![0.0_f64; h + 1];
    for x in 0..w {
        col_prefix[0] = 0.0;
        for y in 0..h {
            col_prefix[y + 1] = col_prefix[y] + (tmp[y * w + x] as f64);
        }
        for y in 0..h {
            let y0 = y.saturating_sub(r);
            let y1 = (y + r).min(h - 1);
            let cnt = (y1 - y0 + 1) as f64;
            dst[y * w + x] = ((col_prefix[y1 + 1] - col_prefix[y0]) / cnt) as f32;
        }
    }
    dst
}

/// He et al. Color-Guided Alpha Filter restricted to the boundary uncertainty trimap band.
/// Snaps blurry / slightly fat neural segmentation edges strictly onto the full-resolution RGB color boundary
/// while leaving deep opaque cores, interior highlights, and rescued VFX glow untouched.
fn refine_alpha_boundary_guided(
    raw_rgb: &[u8],
    alpha_in: &[f32],
    vfx_mask: &[bool],
    w: usize,
    h: usize,
) -> Vec<f32> {
    let n = w * h;
    if w < 24 || h < 24 || raw_rgb.len() < n * 3 || alpha_in.len() < n {
        return alpha_in.to_vec();
    }

    let r_box = (w.min(h) / 200).clamp(2, 5);
    let eps = 0.0018_f32;

    // Identify narrow transition trimap band (within r_box+1 px of alpha transition or soft 0.03..0.97)
    let high_mask: Vec<bool> = alpha_in.iter().map(|&a| a >= 0.96).collect();
    let low_mask: Vec<bool> = alpha_in.iter().map(|&a| a <= 0.04).collect();
    let dist_to_high = manhattan_distance_map(&high_mask, w, h);
    let dist_to_low = manhattan_distance_map(&low_mask, w, h);
    let band_limit = (r_box as u32) + 1;

    let mut has_band = false;
    let mut in_band = vec![false; n];
    for i in 0..n {
        if !vfx_mask[i]
            && ((alpha_in[i] > 0.02 && alpha_in[i] < 0.98)
                || (dist_to_high[i] <= band_limit && dist_to_low[i] <= band_limit))
        {
            in_band[i] = true;
            has_band = true;
        }
    }
    if !has_band {
        return alpha_in.to_vec();
    }

    // Diagonal 3-channel RGB Guided Filter (captures both luma and pure-chroma boundaries in O(N))
    let mut out_q = vec![0.0_f32; n];
    let mut weight_ch = vec![0.0_f32; n];
    let mean_p = box_mean_2d(alpha_in, w, h, r_box);

    for ch in 0..3 {
        let guide: Vec<f32> = (0..n).map(|i| (raw_rgb[i * 3 + ch] as f32) / 255.0).collect();
        let ip: Vec<f32> = (0..n).map(|i| guide[i] * alpha_in[i]).collect();
        let ii: Vec<f32> = (0..n).map(|i| guide[i] * guide[i]).collect();

        let mean_i = box_mean_2d(&guide, w, h, r_box);
        let mean_ip = box_mean_2d(&ip, w, h, r_box);
        let mean_ii = box_mean_2d(&ii, w, h, r_box);

        let mut a_k = vec![0.0_f32; n];
        let mut b_k = vec![0.0_f32; n];
        let mut var_w = vec![0.0_f32; n];
        for idx in 0..n {
            let cov = mean_ip[idx] - mean_i[idx] * mean_p[idx];
            let var = (mean_ii[idx] - mean_i[idx] * mean_i[idx]).max(0.0);
            let a = cov / (var + eps);
            let b = mean_p[idx] - a * mean_i[idx];
            a_k[idx] = a;
            b_k[idx] = b;
            var_w[idx] = var + 1e-4;
        }

        let mean_a = box_mean_2d(&a_k, w, h, r_box);
        let mean_b = box_mean_2d(&b_k, w, h, r_box);
        let mean_vw = box_mean_2d(&var_w, w, h, r_box);

        for idx in 0..n {
            let w_c = mean_vw[idx];
            let q_c = (mean_a[idx] * guide[idx] + mean_b[idx]).clamp(0.0, 1.0);
            out_q[idx] += q_c * w_c;
            weight_ch[idx] += w_c;
        }
    }

    let mut refined = alpha_in.to_vec();
    for idx in 0..n {
        if in_band[idx] && weight_ch[idx] > 0.0 {
            let q = (out_q[idx] / weight_ch[idx]).clamp(0.0, 1.0);
            refined[idx] = if q < 0.015 {
                0.0
            } else if q > 0.992 {
                1.0
            } else {
                q
            };
        }
    }
    refined
}

/// Fast Multi-Level Foreground & Background Estimation (Germer et al., PyMatting `estimate_foreground_ml`).
/// Solves the coupled smoothness-regularized compositing equation `I = α F + (1 - α) B` on a coarse-to-fine
/// image pyramid, simultaneously recovering halo-free foreground colors `F` across all semi-transparent
/// hair/edge pixels AND naturally bleeding clean foreground RGB into adjacent transparent pixels.
fn estimate_foreground_multilevel(
    raw_rgb: &[u8],
    alpha: &[f32],
    w: usize,
    h: usize,
) -> (Vec<f32>, Vec<f32>) {
    fn solve_level(
        img_rgb: &[f32],
        a_lvl: &[f32],
        lw: usize,
        lh: usize,
        mut fg: Vec<f32>,
        mut bg: Vec<f32>,
        sweeps: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let reg_base = 0.12_f32;
        for _ in 0..sweeps {
            for y in 0..lh {
                let row = y * lw;
                for x in 0..lw {
                    let idx = row + x;
                    let a = a_lvl[idx].clamp(0.0, 1.0);
                    let b = 1.0 - a;
                    let reg_f = reg_base + b * 0.28;
                    let reg_b = reg_base + a * 0.28;

                    let mut nb_cnt = 0.0_f32;
                    let mut sf = [0.0_f32; 3];
                    let mut sb = [0.0_f32; 3];

                    if x > 0 {
                        let ni = (idx - 1) * 3;
                        nb_cnt += 1.0;
                        for c in 0..3 {
                            sf[c] += fg[ni + c];
                            sb[c] += bg[ni + c];
                        }
                    }
                    if x + 1 < lw {
                        let ni = (idx + 1) * 3;
                        nb_cnt += 1.0;
                        for c in 0..3 {
                            sf[c] += fg[ni + c];
                            sb[c] += bg[ni + c];
                        }
                    }
                    if y > 0 {
                        let ni = (idx - lw) * 3;
                        nb_cnt += 1.0;
                        for c in 0..3 {
                            sf[c] += fg[ni + c];
                            sb[c] += bg[ni + c];
                        }
                    }
                    if y + 1 < lh {
                        let ni = (idx + lw) * 3;
                        nb_cnt += 1.0;
                        for c in 0..3 {
                            sf[c] += fg[ni + c];
                            sb[c] += bg[ni + c];
                        }
                    }

                    let a11 = a * a + nb_cnt * reg_f;
                    let a22 = b * b + nb_cnt * reg_b;
                    let a12 = a * b;
                    let det = (a11 * a22 - a12 * a12).max(1e-6);
                    let inv_det = 1.0 / det;

                    let p3 = idx * 3;
                    for c in 0..3 {
                        let ic = img_rgb[p3 + c];
                        let rhs1 = a * ic + reg_f * sf[c];
                        let rhs2 = b * ic + reg_b * sb[c];
                        fg[p3 + c] = ((a22 * rhs1 - a12 * rhs2) * inv_det).clamp(0.0, 255.0);
                        bg[p3 + c] = ((a11 * rhs2 - a12 * rhs1) * inv_det).clamp(0.0, 255.0);
                    }
                }
            }
        }
        (fg, bg)
    }

    fn downsample_2x_rgb_alpha(
        rgb: &[f32],
        a: &[f32],
        sw: usize,
        sh: usize,
    ) -> (Vec<f32>, Vec<f32>, usize, usize) {
        let dw = (sw / 2).max(1);
        let dh = (sh / 2).max(1);
        let mut d_rgb = vec![0.0_f32; dw * dh * 3];
        let mut d_a = vec![0.0_f32; dw * dh];
        for dy in 0..dh {
            let sy0 = (dy * 2).min(sh - 1);
            let sy1 = (sy0 + 1).min(sh - 1);
            for dx in 0..dw {
                let sx0 = (dx * 2).min(sw - 1);
                let sx1 = (sx0 + 1).min(sw - 1);
                let idxs = [sy0 * sw + sx0, sy0 * sw + sx1, sy1 * sw + sx0, sy1 * sw + sx1];
                let out_i = dy * dw + dx;
                d_a[out_i] = 0.25 * (a[idxs[0]] + a[idxs[1]] + a[idxs[2]] + a[idxs[3]]);
                for c in 0..3 {
                    d_rgb[out_i * 3 + c] = 0.25
                        * (rgb[idxs[0] * 3 + c]
                            + rgb[idxs[1] * 3 + c]
                            + rgb[idxs[2] * 3 + c]
                            + rgb[idxs[3] * 3 + c]);
                }
            }
        }
        (d_rgb, d_a, dw, dh)
    }

    fn upsample_rgb_bilinear(src: &[f32], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<f32> {
        let mut dst = vec![0.0_f32; dw * dh * 3];
        let scale_x = (sw as f32) / (dw as f32);
        let scale_y = (sh as f32) / (dh as f32);
        for dy in 0..dh {
            let fy = ((dy as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (sh - 1) as f32);
            let y0 = fy.floor() as usize;
            let y1 = (y0 + 1).min(sh - 1);
            let wy = fy - (y0 as f32);
            for dx in 0..dw {
                let fx = ((dx as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (sw - 1) as f32);
                let x0 = fx.floor() as usize;
                let x1 = (x0 + 1).min(sw - 1);
                let wx = fx - (x0 as f32);
                let i00 = (y0 * sw + x0) * 3;
                let i10 = (y0 * sw + x1) * 3;
                let i01 = (y1 * sw + x0) * 3;
                let i11 = (y1 * sw + x1) * 3;
                let out_i = (dy * dw + dx) * 3;
                for c in 0..3 {
                    let top = src[i00 + c] + (src[i10 + c] - src[i00 + c]) * wx;
                    let bot = src[i01 + c] + (src[i11 + c] - src[i01 + c]) * wx;
                    dst[out_i + c] = top + (bot - top) * wy;
                }
            }
        }
        dst
    }

    let n = w * h;
    let base_rgb: Vec<f32> = raw_rgb.iter().take(n * 3).map(|&v| v as f32).collect();
    let mut levels: Vec<(Vec<f32>, Vec<f32>, usize, usize)> = vec![(base_rgb, alpha.to_vec(), w, h)];

    while {
        let (_, _, cw, ch) = levels.last().unwrap();
        (*cw).min(*ch) > 32
    } {
        let (prgb, pa, cw, ch) = levels.last().unwrap();
        let next = downsample_2x_rgb_alpha(prgb, pa, *cw, *ch);
        levels.push(next);
    }

    let (coarsest_rgb, coarsest_a, cw, ch) = levels.last().unwrap();
    let (mut fg, mut bg) = solve_level(
        coarsest_rgb,
        coarsest_a,
        *cw,
        *ch,
        coarsest_rgb.clone(),
        coarsest_rgb.clone(),
        10,
    );

    for lvl in (0..levels.len() - 1).rev() {
        let (_, _, pw, ph) = levels[lvl + 1];
        let (ref l_rgb, ref l_a, nw, nh) = levels[lvl];
        let up_fg = upsample_rgb_bilinear(&fg, pw, ph, nw, nh);
        let up_bg = upsample_rgb_bilinear(&bg, pw, ph, nw, nh);
        let sweeps = if nw.max(nh) >= 512 { 3 } else { 5 };
        let solved = solve_level(l_rgb, l_a, nw, nh, up_fg, up_bg, sweeps);
        fg = solved.0;
        bg = solved.1;
    }

    (fg, bg)
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

    let mut a2 = refine_alpha_boundary_guided(raw_rgb, &alpha_src, &vec![false; n], w, h);

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

    // 3-pass dilation/bridge for luminous white-hot VFX cores (e.g. crescent slash interior)
    for _pass in 0..3 {
        let prev_vfx = vfx_mask.clone();
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let idx = row + x;
                if prev_vfx[idx] {
                    continue;
                }
                if !(c_max_arr[idx] > 185.0 && alpha_src[idx] > 0.35) {
                    continue;
                }
                let mut adj_vfx = false;
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
                        if prev_vfx[(ny as usize) * w + (nx as usize)] {
                            adj_vfx = true;
                            break;
                        }
                    }
                    if adj_vfx {
                        break;
                    }
                }
                let mut bridge_opp = false;
                if !adj_vfx {
                    let has_left = (1..=6_usize).any(|d| x >= d && prev_vfx[row + (x - d)]);
                    let has_right = (1..=6_usize).any(|d| x + d < w && prev_vfx[row + (x + d)]);
                    let has_up = (1..=6_usize).any(|d| y >= d && prev_vfx[(y - d) * w + x]);
                    let has_down = (1..=6_usize).any(|d| y + d < h && prev_vfx[(y + d) * w + x]);
                    bridge_opp = (has_left && has_right) || (has_up && has_down);
                }
                if adj_vfx || bridge_opp {
                    vfx_mask[idx] = true;
                }
            }
        }
    }

    // Restore original soft neural alpha on detected VFX glow regions before rescue
    for i in 0..n {
        if vfx_mask[i] && alpha_src[i] > a2[i] {
            a2[i] = alpha_src[i];
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

    let has_vfx_within_2px = |pidx: usize| -> bool {
        let px = (pidx % w) as isize;
        let py = (pidx / w) as isize;
        for dy in -2_isize..=2 {
            let ny = py + dy;
            if ny < 0 || ny >= h as isize {
                continue;
            }
            for dx in -2_isize..=2 {
                let nx = px + dx;
                if nx < 0 || nx >= w as isize {
                    continue;
                }
                if vfx_mask[(ny as usize) * w + (nx as usize)] {
                    return true;
                }
            }
        }
        false
    };

    if is_light_neutral_bg {
        // 4-connected BFS avoids leaking diagonally across 1px dark outlines around white eyes/armor.
        let mut pocket_visited = vec![false; n];
        let mut pocket_queue = VecDeque::new();
        for i in 0..n {
            if vfx_mask[i] {
                continue;
            }
            if alpha_src[i] > 0.60 && c_max_arr[i] > 195.0 && has_vfx_within_2px(i) {
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
                if alpha_src[nidx] > 0.60 && c_max_arr[nidx] > 195.0 && has_vfx_within_2px(nidx) {
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
                    || (c_max_arr[i] > 165.0 && a2[i] > 0.05 && d_pc_arr[i] <= 225.0))
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

    // Isotropic Sub-Pixel Edge Choke (0..=5 px 8-neighbor level-set erosion: eliminates 4-neighbor diamond stair-stepping)
    if choke_iters > 0 {
        let mut next_a = a2.clone();
        let diag_step = std::f32::consts::FRAC_1_SQRT_2; // 0.70710678
        for _ in 0..choke_iters {
            next_a.copy_from_slice(&a2);
            for y in 0..h {
                let row = y * w;
                for x in 0..w {
                    let idx = row + x;
                    if thin_protect[idx] || vfx_protected[idx] {
                        continue;
                    }
                    let c = a2[idx];
                    let mut m = c;
                    if x > 0 {
                        m = m.min(a2[idx - 1]);
                    }
                    if x + 1 < w {
                        m = m.min(a2[idx + 1]);
                    }
                    if y > 0 {
                        m = m.min(a2[idx - w]);
                    }
                    if y + 1 < h {
                        m = m.min(a2[idx + w]);
                    }
                    // Diagonal neighbors scaled by 1/sqrt(2) distance for circular isotropy
                    for (dx, dy) in [(-1_isize, -1_isize), (1, -1), (-1, 1), (1, 1)] {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                            let nd = a2[(ny as usize) * w + (nx as usize)];
                            if nd < c {
                                let diag_proj = c + (nd - c) * diag_step;
                                m = m.min(diag_proj);
                            }
                        }
                    }
                    next_a[idx] = m.clamp(0.0, 1.0);
                }
            }
            std::mem::swap(&mut a2, &mut next_a);
        }
    }

    // 4b. Collinear Rim Decontamination: fix fat neural mask pixels (0.35 <= α <= 1.0) right on the outer border
    // that are actually linear blends between interior foreground core and background (the #1 cause of white sticker halos!)
    if w >= 12 && h >= 12 {
        let snap_a = a2.clone();
        for y in 2..h.saturating_sub(2) {
            let row = y * w;
            for x in 2..w.saturating_sub(2) {
                let idx = row + x;
                let a = snap_a[idx];
                if a < 0.35 || vfx_mask[idx] || thin_protect[idx] {
                    continue;
                }
                let touches_soft_or_out = snap_a[idx - 1] < 0.35
                    || snap_a[idx + 1] < 0.35
                    || snap_a[idx - w] < 0.35
                    || snap_a[idx + w] < 0.35;
                if !touches_soft_or_out {
                    continue;
                }

                // Find cleanest deep core pixel (snap_a >= 0.98) within 2px inward
                let mut core_rgb: Option<[f32; 3]> = None;
                let mut max_bg_dist = -1.0_f32;
                for (dx, dy) in [
                    (-2_isize, 0_isize),
                    (2, 0),
                    (0, -2),
                    (0, 2),
                    (-2, -2),
                    (2, -2),
                    (-2, 2),
                    (2, 2),
                ] {
                    let nx = (x as isize + dx) as usize;
                    let ny = (y as isize + dy) as usize;
                    let nidx = ny * w + nx;
                    if snap_a[nidx] >= 0.98 && d_pc_arr[nidx] > max_bg_dist {
                        max_bg_dist = d_pc_arr[nidx];
                        core_rgb = Some([
                            raw_rgb[nidx * 3] as f32,
                            raw_rgb[nidx * 3 + 1] as f32,
                            raw_rgb[nidx * 3 + 2] as f32,
                        ]);
                    }
                }

                if let Some(fc) = core_rgb {
                    let vx = fc[0] - bgc[0];
                    let vy = fc[1] - bgc[1];
                    let vz = fc[2] - bgc[2];
                    let seg_sq = vx * vx + vy * vy + vz * vz;
                    if seg_sq >= 2025.0 {
                        // |F_core - bgc| >= 45
                        let px_r = raw_rgb[idx * 3] as f32;
                        let px_g = raw_rgb[idx * 3 + 1] as f32;
                        let px_b = raw_rgb[idx * 3 + 2] as f32;
                        let wx = px_r - bgc[0];
                        let wy = px_g - bgc[1];
                        let wz = px_b - bgc[2];
                        let t = (wx * vx + wy * vy + wz * vz) / seg_sq;
                        if (0.12..=0.92).contains(&t) {
                            let proj_r = bgc[0] + t * vx;
                            let proj_g = bgc[1] + t * vy;
                            let proj_b = bgc[2] + t * vz;
                            let perp_sq = (px_r - proj_r).powi(2)
                                + (px_g - proj_g).powi(2)
                                + (px_b - proj_b).powi(2);
                            if perp_sq <= seg_sq * 0.025 {
                                a2[idx] = a.min(t);
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. PyMatting-style Multi-Level Foreground Estimation + Automatic Transparent Color Bleed
    // Replaces single-pixel division & white fade-back so even α=0.04 hair tips retain pure unmixed foreground RGB!
    let (mut rgb_f32, _bg_f32) = estimate_foreground_multilevel(raw_rgb, &a2, w, h);

    // Ensure completely opaque core pixels (a2 >= 0.995) remain 100% bitwise identical to original RGB
    for i in 0..n {
        if a2[i] >= 0.995 {
            rgb_f32[i * 3] = raw_rgb[i * 3] as f32;
            rgb_f32[i * 3 + 1] = raw_rgb[i * 3 + 1] as f32;
            rgb_f32[i * 3 + 2] = raw_rgb[i * 3 + 2] as f32;
        }
    }

    // 6. Reinforce 4px crisp color bleed from solid rim into immediate transparent neighbors for bilinear game engines
    let mut solid: Vec<bool> = (0..n)
        .map(|i| a2[i] > 0.55 || (vfx_mask[i] && a2[i] > 0.06))
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
                if solid[idx] || a2[idx] > 0.02 {
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

/// Detect whether an image's outer perimeter is predominantly light (`true`, e.g. #FFFFFF) or dark (`false`, e.g. #000000).
pub fn is_light_backdrop_plate(img: &DynamicImage) -> bool {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    if w == 0 || h == 0 {
        return true;
    }
    let mut lumas = Vec::new();
    for x in 0..w {
        for y in [0, h.saturating_sub(1)] {
            let p = rgb.get_pixel(x, y);
            lumas.push(0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32);
        }
    }
    for y in 0..h {
        for x in [0, w.saturating_sub(1)] {
            let p = rgb.get_pixel(x, y);
            lumas.push(0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32);
        }
    }
    median_f32(&mut lumas) >= 128.0
}

/// Smith-Blinn Closed-Form Dual-Background Triangulation Matting (`light_plate` + `dark_plate`).
/// Auto-registers sub-pixel/integer translation shifts (`±6px`) caused by image-edit models, then
/// solves the overdetermined linear system `I_W = α F + (1 - α) B_W` & `I_K = α F + (1 - α) B_K`
/// for physically exact translucent alpha (glass, fire, smoke, lace, hair) with zero white halo.
pub fn triangulate_dual_bg_rgba(
    light_img: &DynamicImage,
    dark_img: &DynamicImage,
    choke: u8,
) -> RgbaImage {
    let (w_u32, h_u32) = light_img.dimensions();
    let w = w_u32 as usize;
    let h = h_u32 as usize;
    if w == 0 || h == 0 {
        return RgbaImage::new(w_u32, h_u32);
    }

    let light_rgb = light_img.to_rgb8();
    let dark_resized = if dark_img.dimensions() == (w_u32, h_u32) {
        dark_img.to_rgb8()
    } else {
        dark_img
            .resize_exact(w_u32, h_u32, FilterType::Lanczos3)
            .to_rgb8()
    };

    let lw_raw = light_rgb.as_raw();
    let dk_raw = dark_resized.as_raw();

    // 1. Estimate true light & dark backdrop colors from outer border median
    let sample_border_median = |raw: &[u8]| -> [f32; 3] {
        let mut rs = Vec::new();
        let mut gs = Vec::new();
        let mut bs = Vec::new();
        let mut push = |x: usize, y: usize| {
            let i = (y * w + x) * 3;
            rs.push(raw[i] as f32);
            gs.push(raw[i + 1] as f32);
            bs.push(raw[i + 2] as f32);
        };
        for x in 0..w {
            push(x, 0);
            if h > 1 {
                push(x, h - 1);
            }
        }
        for y in 0..h {
            push(0, y);
            if w > 1 {
                push(w - 1, y);
            }
        }
        [median_f32(&mut rs), median_f32(&mut gs), median_f32(&mut bs)]
    };

    let bw_col = sample_border_median(lw_raw);
    let bk_col = sample_border_median(dk_raw);

    // 2. Align integer translation shift (dx, dy) in [-6..=6] via interior gradient correlation
    let (best_dx, best_dy) = if w >= 32 && h >= 32 {
        let step = (w.min(h) / 256).max(1);
        let x0 = w / 8;
        let x1 = w - x0;
        let y0 = h / 8;
        let y1 = h - y0;
        let mut best_shift = (0_isize, 0_isize);
        let mut min_err = f64::INFINITY;

        for dy in -6_isize..=6 {
            for dx in -6_isize..=6 {
                let mut err_sum = 0.0_f64;
                let mut count = 0_usize;
                let mut y = y0;
                while y + 1 < y1 {
                    let ny = (y as isize + dy).clamp(0, (h - 2) as isize) as usize;
                    let mut x = x0;
                    while x + 1 < x1 {
                        let nx = (x as isize + dx).clamp(0, (w - 2) as isize) as usize;
                        let il = (y * w + x) * 3;
                        let ik = (ny * w + nx) * 3;
                        // Compare interior horizontal & vertical spatial derivatives (invariant to constant background where foreground is solid!)
                        let d_lw = (lw_raw[il] as f32 - bw_col[0]).abs()
                            + (lw_raw[il + 1] as f32 - bw_col[1]).abs()
                            + (lw_raw[il + 2] as f32 - bw_col[2]).abs();
                        if d_lw > 36.0 {
                            let glx = (lw_raw[il + 3] as i32 - lw_raw[il] as i32).abs();
                            let gkx = (dk_raw[ik + 3] as i32 - dk_raw[ik] as i32).abs();
                            let gly = (lw_raw[il + w * 3] as i32 - lw_raw[il] as i32).abs();
                            let gky = (dk_raw[ik + w * 3] as i32 - dk_raw[ik] as i32).abs();
                            err_sum += ((glx - gkx).abs() + (gly - gky).abs()) as f64;
                            count += 1;
                        }
                        x += step;
                    }
                    y += step;
                }
                if count >= 16 {
                    // Slight preference for (0,0) when tied
                    let reg = ((dx.abs() + dy.abs()) as f64) * 0.15;
                    let avg_err = err_sum / (count as f64) + reg;
                    if avg_err < min_err {
                        min_err = avg_err;
                        best_shift = (dx, dy);
                    }
                }
            }
        }
        best_shift
    } else {
        (0, 0)
    };

    // 3. Smith-Blinn closed-form least-squares alpha & un-premultiplied foreground recovery
    let db_r = (bw_col[0] - bk_col[0]).max(40.0);
    let db_g = (bw_col[1] - bk_col[1]).max(40.0);
    let db_b = (bw_col[2] - bk_col[2]).max(40.0);
    let db_norm_sq = db_r * db_r + db_g * db_g + db_b * db_b;

    let n = w * h;
    let mut alpha = vec![0.0_f32; n];
    let mut fg_rgb = vec![0.0_f32; n * 3];

    for y in 0..h {
        let ny = (y as isize + best_dy).clamp(0, (h - 1) as isize) as usize;
        for x in 0..w {
            let nx = (x as isize + best_dx).clamp(0, (w - 1) as isize) as usize;
            let idx = y * w + x;
            let il = idx * 3;
            let ik = (ny * w + nx) * 3;

            let wr = lw_raw[il] as f32;
            let wg = lw_raw[il + 1] as f32;
            let wb = lw_raw[il + 2] as f32;

            let kr = dk_raw[ik] as f32;
            let kg = dk_raw[ik + 1] as f32;
            let kb = dk_raw[ik + 2] as f32;

            // Difference vector ΔI = I_W - I_K projected onto ΔB = B_W - B_K
            let di_r = wr - kr;
            let di_g = wg - kg;
            let di_b = wb - kb;
            let dot = di_r * db_r + di_g * db_g + di_b * db_b;
            let mut a = (1.0 - dot / db_norm_sq).clamp(0.0, 1.0);

            // Additive luminous glow rescue: if pixel on dark plate glows brightly even where I_W saturated to 255
            let dist_k_bg =
                ((kr - bk_col[0]).powi(2) + (kg - bk_col[1]).powi(2) + (kb - bk_col[2]).powi(2))
                    .sqrt();
            let dist_w_bg =
                ((wr - bw_col[0]).powi(2) + (wg - bw_col[1]).powi(2) + (wb - bw_col[2]).powi(2))
                    .sqrt();

            if dist_k_bg < 14.0 && dist_w_bg < 14.0 {
                a = 0.0;
            } else if a > 0.94 && dist_k_bg > 26.0 && dist_w_bg > 26.0 {
                a = 1.0;
            } else if a < 0.025 {
                a = 0.0;
            }

            alpha[idx] = a;

            if a >= 0.96 {
                // Opaque core: keep original light-plate RGB for 100% fidelity to user's original art
                fg_rgb[il] = wr;
                fg_rgb[il + 1] = wg;
                fg_rgb[il + 2] = wb;
            } else if a > 0.0 {
                // Translucent / edge pixel: unmix directly from the dark plate (ZERO white halo contamination!)
                let denom = a.max(0.06);
                let ur = ((kr - (1.0 - a) * bk_col[0]) / denom).clamp(0.0, 255.0);
                let ug = ((kg - (1.0 - a) * bk_col[1]) / denom).clamp(0.0, 255.0);
                let ub = ((kb - (1.0 - a) * bk_col[2]) / denom).clamp(0.0, 255.0);
                let w_orig = ((a - 0.80) / 0.16).clamp(0.0, 1.0);
                fg_rgb[il] = ur * (1.0 - w_orig) + wr * w_orig;
                fg_rgb[il + 1] = ug * (1.0 - w_orig) + wg * w_orig;
                fg_rgb[il + 2] = ub * (1.0 - w_orig) + wb * w_orig;
            }
        }
    }

    // Optional isotropic choke if requested
    let choke_iters = choke.min(5) as usize;
    if choke_iters > 0 {
        let diag_step = std::f32::consts::FRAC_1_SQRT_2;
        let mut next_a = alpha.clone();
        for _ in 0..choke_iters {
            next_a.copy_from_slice(&alpha);
            for y in 0..h {
                let row = y * w;
                for x in 0..w {
                    let idx = row + x;
                    let c = alpha[idx];
                    if c <= 0.0 || c < 0.82 {
                        // Preserve genuine semi-transparent glass/smoke/glow! Only choke hard edges.
                        continue;
                    }
                    let mut m = c;
                    if x > 0 {
                        m = m.min(alpha[idx - 1]);
                    }
                    if x + 1 < w {
                        m = m.min(alpha[idx + 1]);
                    }
                    if y > 0 {
                        m = m.min(alpha[idx - w]);
                    }
                    if y + 1 < h {
                        m = m.min(alpha[idx + w]);
                    }
                    for (dx, dy) in [(-1_isize, -1_isize), (1, -1), (-1, 1), (1, 1)] {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                            let nd = alpha[(ny as usize) * w + (nx as usize)];
                            if nd < c {
                                m = m.min(c + (nd - c) * diag_step);
                            }
                        }
                    }
                    next_a[idx] = m.clamp(0.0, 1.0);
                }
            }
            std::mem::swap(&mut alpha, &mut next_a);
        }
    }

    // 4px engine color bleed into transparent pixels
    let mut solid: Vec<bool> = alpha.iter().map(|&a| a > 0.05).collect();
    let mut next_rgb = fg_rgb.clone();
    let mut next_solid = solid.clone();
    for _ in 0..4 {
        next_rgb.copy_from_slice(&fg_rgb);
        next_solid.copy_from_slice(&solid);
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let idx = row + x;
                if solid[idx] {
                    continue;
                }
                let mut sr = 0.0;
                let mut sg = 0.0;
                let mut sb = 0.0;
                let mut wt = 0.0;
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
                            sr += fg_rgb[nidx * 3] * w_nb;
                            sg += fg_rgb[nidx * 3 + 1] * w_nb;
                            sb += fg_rgb[nidx * 3 + 2] * w_nb;
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
        std::mem::swap(&mut fg_rgb, &mut next_rgb);
        std::mem::swap(&mut solid, &mut next_solid);
    }

    let mut out = RgbaImage::new(w_u32, h_u32);
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            out.put_pixel(
                x as u32,
                y as u32,
                Rgba([
                    fg_rgb[idx * 3].round().clamp(0.0, 255.0) as u8,
                    fg_rgb[idx * 3 + 1].round().clamp(0.0, 255.0) as u8,
                    fg_rgb[idx * 3 + 2].round().clamp(0.0, 255.0) as u8,
                    (alpha[idx] * 255.0).round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    out
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

    #[test]
    fn test_crescent_slash_white_hot_core_not_hollowed() {
        let w = 48_usize;
        let h = 48_usize;
        let mut rgb = RgbImage::from_pixel(w as u32, h as u32, Rgb([255, 255, 255]));
        let mut alpha = vec![0.0_f32; w * h];

        // Opaque character body at left
        for y in 16..32 {
            for x in 6..14 {
                rgb.put_pixel(x as u32, y as u32, Rgb([45, 55, 90]));
                alpha[y * w + x] = 1.0;
            }
        }
        // Crescent slash: cyan outer rim (x=20..22 and x=27..29), white-hot low-chroma core (x=23..26)
        for y in 12..36 {
            for x in 20..30 {
                if x <= 21 || x >= 28 {
                    rgb.put_pixel(x as u32, y as u32, Rgb([40, 215, 255]));
                    alpha[y * w + x] = 0.88;
                } else {
                    // White-hot interior with very low chroma and soft alpha < 0.85
                    rgb.put_pixel(x as u32, y as u32, Rgb([250, 254, 255]));
                    alpha[y * w + x] = 0.76;
                }
            }
        }

        let res = compose_rgba(&DynamicImage::ImageRgb8(rgb), &alpha, true, 1);
        for y in 16..32 {
            for x in 23..27 {
                let a = res.get_pixel(x as u32, y as u32)[3];
                assert!(
                    a >= 160,
                    "white-hot slash core at ({}, {}) was hollowed out: alpha={}",
                    x,
                    y,
                    a
                );
            }
        }
    }

    #[test]
    fn test_fat_mask_halo_decontamination_and_dual_plate_triangulation() {
        // Part A: Fat neural mask (alpha=1.0 on a 50% dark-hair / 50% white-bg anti-aliased edge pixel)
        let mut rgb = RgbImage::from_pixel(40, 40, Rgb([255, 255, 255]));
        let mut alpha = vec![0.0_f32; 40 * 40];
        for y in 10..30 {
            for x in 10..30 {
                let is_aa_rim = x == 10 || x == 29 || y == 10 || y == 29;
                if is_aa_rim {
                    // 50% blend of dark navy [20, 30, 50] and white [255, 255, 255] = [138, 143, 153]
                    rgb.put_pixel(x, y, Rgb([138, 143, 153]));
                    alpha[(y * 40 + x) as usize] = 1.0; // Fat binary mask bug!
                } else {
                    rgb.put_pixel(x, y, Rgb([20, 30, 50]));
                    alpha[(y * 40 + x) as usize] = 1.0;
                }
            }
        }
        let res = compose_rgba(&DynamicImage::ImageRgb8(rgb), &alpha, false, 0);
        // Guided filter + Collinear rim decontamination must drop the fat 1.0 rim alpha below 185 AND recover dark RGB!
        let rim_px = res.get_pixel(10, 20);
        assert!(
            rim_px[3] < 185,
            "Fat mask white-contaminated rim stayed opaque: {:?}",
            rim_px
        );
        assert!(
            rim_px[0] < 90 && rim_px[1] < 100,
            "Multi-level foreground estimation failed to strip white halo from rim RGB: {:?}",
            rim_px
        );

        // Part B: Smith-Blinn Dual-Plate Triangulation with a +2px horizontal shift on the dark plate!
        // Includes: 100% opaque pure-white hair [252,252,255], 40% translucent cyan glass potion [60,210,245], and pure backdrop
        let mut plate_w = RgbImage::from_pixel(48, 48, Rgb([255, 255, 255]));
        let mut plate_k = RgbImage::from_pixel(48, 48, Rgb([0, 0, 0]));
        let shift_x = 2_u32;

        // Opaque dark outline + pure white hair (x=12..22, y=12..24)
        for y in 12..24 {
            for x in 12..22 {
                let c = if x == 12 || x == 21 || y == 12 || y == 23 {
                    [28, 32, 42]
                } else {
                    [252, 252, 255]
                };
                plate_w.put_pixel(x, y, Rgb(c));
                plate_k.put_pixel(x + shift_x, y, Rgb(c));
            }
        }
        // 40% translucent cyan glass potion (x=26..36, y=14..28), true F=[50, 200, 240], alpha=0.40
        for y in 14..28 {
            for x in 26..36 {
                let a = 0.40_f32;
                let f = [50.0_f32, 200.0, 240.0];
                let cw = [
                    (a * f[0] + (1.0 - a) * 255.0).round() as u8,
                    (a * f[1] + (1.0 - a) * 255.0).round() as u8,
                    (a * f[2] + (1.0 - a) * 255.0).round() as u8,
                ];
                let ck = [
                    (a * f[0]).round() as u8,
                    (a * f[1]).round() as u8,
                    (a * f[2]).round() as u8,
                ];
                plate_w.put_pixel(x, y, Rgb(cw));
                plate_k.put_pixel(x + shift_x, y, Rgb(ck));
            }
        }

        let tri = triangulate_dual_bg_rgba(
            &DynamicImage::ImageRgb8(plate_w),
            &DynamicImage::ImageRgb8(plate_k),
            0,
        );
        // White hair must remain 100% opaque white!
        let hair = tri.get_pixel(16, 18);
        assert!(hair[3] >= 250 && hair[0] >= 245, "White hair lost: {:?}", hair);
        // 40% cyan glass must recover alpha ~102 (40% of 255) and unmixed cyan RGB [~50, ~200, ~240] (NOT white-tinted [173, 233, 249]!)
        let glass = tri.get_pixel(30, 20);
        assert!(
            (glass[3] as i32 - 102).abs() <= 10,
            "Expected ~40% alpha (102), got {:?}",
            glass
        );
        assert!(
            glass[0] < 80 && glass[2] > 215,
            "Glass RGB still contaminated with white: {:?}",
            glass
        );
        // Background stays 0 alpha
        assert_eq!(tri.get_pixel(4, 4)[3], 0);
    }
}
