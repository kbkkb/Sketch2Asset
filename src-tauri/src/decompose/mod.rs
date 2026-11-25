pub mod downloader;
pub mod worker_template;

use image::{GenericImageView, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use crate::error::{AppError, AppErrorCode, AppResult};
use crate::matting;
use crate::state::AppState;

static INSTALL_CANCEL_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static RUN_CANCEL_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static ACTIVE_RUN_PID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();
static DECOMPOSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn install_cancel_flag() -> Arc<AtomicBool> {
    INSTALL_CANCEL_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn run_cancel_flag() -> Arc<AtomicBool> {
    RUN_CANCEL_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn active_run_pid_lock() -> &'static Mutex<Option<u32>> {
    ACTIVE_RUN_PID.get_or_init(|| Mutex::new(None))
}

fn decompose_mutex() -> &'static Mutex<()> {
    DECOMPOSE_LOCK.get_or_init(|| Mutex::new(()))
}

pub fn cancel_decompose_install() {
    install_cancel_flag().store(true, Ordering::Relaxed);
}

pub fn cancel_decompose_run() {
    run_cancel_flag().store(true, Ordering::Relaxed);
    let pid_opt = active_run_pid_lock().lock().ok().and_then(|g| *g);
    if let Some(pid) = pid_opt {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = Command::new("taskkill");
            cmd.args(["/F", "/T", "/PID", &pid.to_string()]);
            cmd.creation_flags(0x08000000);
            let _ = cmd.output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .output();
        }
    }
}

struct ActivePidGuard;

impl Drop for ActivePidGuard {
    fn drop(&mut self) {
        if let Ok(mut g) = active_run_pid_lock().lock() {
            *g = None;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecomposeEnvStatus {
    pub configured_dir: String,
    pub recommended_dir: String,
    pub effective_dir: String,
    pub disk_free_gb: f64,
    pub disk_total_gb: f64,
    pub has_enough_disk: bool,
    pub gpu_name: String,
    pub vram_total_mb: u64,
    pub vram_free_mb: u64,
    pub has_supported_gpu: bool,
    pub runtime_ready: bool,
    pub repo_ready: bool,
    pub models_ready: bool,
    pub ready: bool,
    pub runtime_estimated_gb: f64,
    pub models_estimated_gb: f64,
    pub required_disk_gb: f64,
    pub backend: String,
    pub quant_mode: String,
    pub mirror: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecomposeLayerInfo {
    pub id: String,
    pub name: String,
    pub z_index: i32,
    pub visible: bool,
    pub opacity: f64,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    #[serde(default)]
    pub offset_x: i32,
    #[serde(default)]
    pub offset_y: i32,
    pub image_path: String,
    pub depth_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecomposeManifest {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub source_path: String,
    pub width: u32,
    pub height: u32,
    pub psd_path: String,
    pub reconstruction_path: String,
    pub peak_vram_gb: f64,
    pub elapsed_sec: f64,
    #[serde(default)]
    pub layerdiff_sec: f64,
    #[serde(default)]
    pub marigold_sec: f64,
    #[serde(default)]
    pub face_recovered: bool,
    #[serde(default)]
    pub used_group_offload: bool,
    pub layers: Vec<DecomposeLayerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecomposeJobResult {
    pub job_id: String,
    pub job_dir: String,
    pub project: String,
    pub source_path: String,
    pub manifest_path: String,
    pub psd_path: String,
    pub reconstruction_path: String,
    pub width: u32,
    pub height: u32,
    pub peak_vram_gb: f64,
    pub elapsed_sec: f64,
    pub layers: Vec<DecomposeLayerInfo>,
    pub asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecomposeHistoryItem {
    pub job_id: String,
    pub job_dir: String,
    pub manifest_path: String,
    pub psd_path: String,
    pub reconstruction_path: String,
    pub width: u32,
    pub height: u32,
    pub layer_count: usize,
    pub created_ts: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDecomposeOptions {
    pub input_path: String,
    pub project: Option<String>,
    pub resolution: Option<u32>,
    pub steps: Option<u32>,
    pub seed: Option<u64>,
    pub quant_mode: Option<String>,
    pub tblr_split: Option<bool>,
    pub auto_remove_bg: Option<bool>,
    pub cpu_offload: Option<bool>,
    #[serde(default)]
    pub group_offload: Option<String>,
    #[serde(default)]
    pub cool_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgressPayload {
    stage: String,
    percent: f64,
    downloaded_mb: f64,
    total_mb: f64,
    speed_mb_s: f64,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunProgressPayload {
    stage: String,
    percent: u32,
    message: String,
}

#[cfg(target_os = "windows")]
pub fn query_disk_space_gb(path: &Path) -> (f64, f64) {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }

    // Walk up to existing ancestor or drive root so non-existent folders still probe drive space
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        if let Some(p) = probe.parent() {
            probe = p.to_path_buf();
        } else {
            break;
        }
    }
    let wide: Vec<u16> = probe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut avail: u64 = 0;
    let mut total: u64 = 0;
    let mut free: u64 = 0;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
    if ok != 0 && total > 0 {
        let gb = 1024.0 * 1024.0 * 1024.0;
        (
            ((avail as f64 / gb) * 10.0).round() / 10.0,
            ((total as f64 / gb) * 10.0).round() / 10.0,
        )
    } else {
        (0.0, 0.0)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn query_disk_space_gb(_path: &Path) -> (f64, f64) {
    (50.0, 100.0)
}

pub fn recommend_extension_dir(app_data_dir: &Path) -> String {
    #[cfg(target_os = "windows")]
    {
        let mut best: Option<(String, f64)> = None;
        for letter in ['D', 'E', 'F', 'G', 'C'] {
            let root = PathBuf::from(format!("{letter}:\\"));
            if root.exists() {
                let (free_gb, _) = query_disk_space_gb(&root);
                // Strongly prefer non-system drive when it has >= 20 GB free
                let score = if letter != 'C' && free_gb >= 20.0 {
                    free_gb + 10_000.0
                } else {
                    free_gb
                };
                if best.as_ref().map(|b| score > b.1).unwrap_or(true) {
                    let dir = if letter == 'C' {
                        app_data_dir
                            .join("extensions")
                            .join("see-through")
                            .to_string_lossy()
                            .replace('\\', "/")
                    } else {
                        format!("{letter}:/Sketch2Asset_Extensions/see-through")
                    };
                    best = Some((dir, score));
                }
            }
        }
        if let Some((dir, _)) = best {
            return dir;
        }
    }
    app_data_dir
        .join("extensions")
        .join("see-through")
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn query_nvidia_gpu() -> (String, u64, u64) {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mut cmd = Command::new("nvidia-smi");
    cmd.args([
        "--query-gpu=name,memory.total,memory.free",
        "--format=csv,noheader,nounits",
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    if let Ok(out) = cmd.output() {
        if out.status.success() {
            let line = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                let total = parts[1].parse::<u64>().unwrap_or(0);
                let free = parts[2].parse::<u64>().unwrap_or(0);
                return (name, total, free);
            }
        }
    }
    (String::new(), 0, 0)
}

fn model_subdir_names(quant_mode: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    if quant_mode == "none" || quant_mode == "bf16" {
        (
            "layerdiff3d",
            "marigold",
            "layerdifforg/seethroughv0.0.2_layerdiff3d",
            "24yearsold/seethroughv0.0.1_marigold",
        )
    } else {
        (
            "layerdiff3d_nf4",
            "marigold_nf4",
            "24yearsold/seethroughv0.0.2_layerdiff3d_nf4",
            "24yearsold/seethroughv0.0.1_marigold_nf4",
        )
    }
}

pub fn inspect_dir_readiness(ext_dir: &Path, quant_mode: &str) -> (bool, bool, bool) {
    let py = ext_dir.join(".venv").join("Scripts").join("python.exe");
    let runtime_ready = py.is_file()
        && ext_dir
            .join(".venv")
            .join("Lib")
            .join("site-packages")
            .join("diffusers")
            .exists()
        && ext_dir
            .join(".venv")
            .join("Lib")
            .join("site-packages")
            .join("psd_tools")
            .exists();

    let repo_ready = ext_dir
        .join("see-through")
        .join("common")
        .join("utils")
        .join("inference_utils.py")
        .is_file()
        && ext_dir
            .join("see-through")
            .join("inference")
            .join("scripts")
            .join("inference_psd_quantized.py")
            .is_file();

    let (ld_sub, mg_sub, _, _) = model_subdir_names(quant_mode);
    let ld_unet = ext_dir
        .join("models")
        .join(ld_sub)
        .join("unet")
        .join("diffusion_pytorch_model.safetensors");
    let ld_vae = ext_dir
        .join("models")
        .join(ld_sub)
        .join("trans_vae")
        .join("diffusion_pytorch_model.safetensors");
    let mg_unet = ext_dir
        .join("models")
        .join(mg_sub)
        .join("unet")
        .join("diffusion_pytorch_model.safetensors");

    let models_ready = ld_unet.metadata().map(|m| m.len() > 100_000_000).unwrap_or(false)
        && ld_vae.metadata().map(|m| m.len() > 50_000_000).unwrap_or(false)
        && mg_unet.metadata().map(|m| m.len() > 100_000_000).unwrap_or(false);

    (runtime_ready, repo_ready, models_ready)
}

pub fn get_decompose_env_status(state: &AppState) -> DecomposeEnvStatus {
    let settings = state.settings.lock().unwrap().clone();
    let recommended_dir = recommend_extension_dir(&state.app_data_dir);
    let configured_dir = settings.decompose_ext_dir.trim().to_string();
    let effective_dir = if configured_dir.is_empty() {
        recommended_dir.clone()
    } else {
        configured_dir.clone()
    };

    let eff_path = PathBuf::from(&effective_dir);
    let (disk_free_gb, disk_total_gb) = query_disk_space_gb(&eff_path);
    let (gpu_name, vram_total_mb, vram_free_mb) = query_nvidia_gpu();
    let (runtime_ready, repo_ready, models_ready) =
        inspect_dir_readiness(&eff_path, &settings.decompose_quant);

    let already_has_any = runtime_ready || models_ready;
    let required_disk_gb = if already_has_any { 1.0 } else { 15.0 };
    let has_enough_disk = disk_free_gb >= required_disk_gb || (runtime_ready && repo_ready && models_ready);
    let is_bf16 = settings.decompose_quant == "bf16" || settings.decompose_quant == "none";

    DecomposeEnvStatus {
        configured_dir,
        recommended_dir,
        effective_dir,
        disk_free_gb,
        disk_total_gb,
        has_enough_disk,
        gpu_name,
        vram_total_mb,
        vram_free_mb,
        has_supported_gpu: vram_total_mb >= 7500,
        runtime_ready,
        repo_ready,
        models_ready,
        ready: runtime_ready && repo_ready && models_ready,
        runtime_estimated_gb: 7.7,
        models_estimated_gb: if is_bf16 { 12.8 } else { 5.3 },
        required_disk_gb,
        backend: settings.decompose_backend,
        quant_mode: settings.decompose_quant,
        mirror: settings.decompose_mirror,
    }
}

pub fn pick_directory_native(initial_dir: Option<&str>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(init) = initial_dir.filter(|s| !s.trim().is_empty()) {
        let p = Path::new(init.trim());
        if p.exists() {
            dialog = dialog.set_directory(p);
        }
    }
    dialog
        .pick_folder()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

pub fn set_decompose_extension_dir(
    state: &AppState,
    dir_path: String,
) -> AppResult<DecomposeEnvStatus> {
    let normalized = dir_path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "Extension directory cannot be empty",
        ));
    }
    {
        let mut guard = state.settings.lock().unwrap();
        guard.decompose_ext_dir = normalized;
    }
    state.persist_settings()?;
    Ok(get_decompose_env_status(state))
}

async fn ensure_uv_executable(ext_dir: &Path, cancel_flag: &Arc<AtomicBool>) -> AppResult<PathBuf> {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mut cmd = Command::new("uv");
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    if cmd.output().map(|o| o.status.success()).unwrap_or(false) {
        return Ok(PathBuf::from("uv"));
    }

    let local_uv = ext_dir.join("tools").join("uv.exe");
    if local_uv.is_file() {
        return Ok(local_uv);
    }

    let tools_dir = ext_dir.join("tools");
    fs::create_dir_all(&tools_dir)?;
    let zip_dest = tools_dir.join("uv-x86_64-pc-windows-msvc.zip");
    let urls = vec![
        "https://ghfast.top/https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-pc-windows-msvc.zip".to_string(),
        "https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-pc-windows-msvc.zip".to_string(),
    ];

    let clients = vec![
        downloader::build_direct_http_client()?,
        downloader::build_proxy_fallback_client()?,
    ];
    let acc = Arc::new(AtomicU64::new(0));
    downloader::download_file_parallel(
        &clients,
        &urls,
        &zip_dest,
        None,
        4,
        cancel_flag,
        &acc,
        &|| {},
    )
    .await?;

    let f = fs::File::open(&zip_dest)?;
    let mut archive = zip::ZipArchive::new(f)
        .map_err(|e| AppError::new(AppErrorCode::Io, format!("Failed to open uv zip: {e}")))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
        if entry.name().ends_with("uv.exe") {
            let mut out = fs::File::create(&local_uv)?;
            std::io::copy(&mut entry, &mut out)?;
            break;
        }
    }
    let _ = fs::remove_file(&zip_dest);
    if !local_uv.is_file() {
        return Err(AppError::new(
            AppErrorCode::Io,
            "Failed to unpack uv.exe into extension tools dir",
        ));
    }
    Ok(local_uv)
}

pub async fn install_decompose_extension(
    app: tauri::AppHandle,
    target_dir: String,
    quant_mode: String,
    mirror: String,
    pypi_mirror: Option<String>,
) -> AppResult<DecomposeEnvStatus> {
    use tauri::{Emitter, Manager};
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let state = app.state::<AppState>();
    let clean_dir = target_dir.trim().replace('\\', "/");
    if clean_dir.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "请先选择拆图扩展安装文件夹",
        ));
    }

    let ext_path = PathBuf::from(&clean_dir);
    fs::create_dir_all(&ext_path).map_err(|e| {
        AppError::new(
            AppErrorCode::Io,
            format!("无法创建或写入扩展目录 {clean_dir}: {e}"),
        )
    })?;

    {
        let mut guard = state.settings.lock().unwrap();
        guard.decompose_ext_dir = clean_dir.clone();
        guard.decompose_quant = if quant_mode == "bf16" || quant_mode == "none" {
            "bf16".to_string()
        } else {
            "nf4".to_string()
        };
        guard.decompose_mirror = if mirror == "huggingface" {
            "huggingface".to_string()
        } else {
            "hf-mirror".to_string()
        };
    }
    state.persist_settings()?;

    let cancel_flag = install_cancel_flag();
    cancel_flag.store(false, Ordering::Relaxed);

    let emit_progress = |stage: &str, pct: f64, dl_mb: f64, tot_mb: f64, spd: f64, msg: &str| {
        let _ = app.emit(
            "decompose-install-progress",
            InstallProgressPayload {
                stage: stage.to_string(),
                percent: (pct * 10.0).round() / 10.0,
                downloaded_mb: (dl_mb * 10.0).round() / 10.0,
                total_mb: (tot_mb * 10.0).round() / 10.0,
                speed_mb_s: (spd * 100.0).round() / 100.0,
                message: msg.to_string(),
            },
        );
    };

    let (runtime_ready_init, repo_ready_init, models_ready_init) =
        inspect_dir_readiness(&ext_path, &quant_mode);

    if !runtime_ready_init || !models_ready_init {
        let (free_gb, _) = query_disk_space_gb(&ext_path);
        if free_gb < 14.0 && !runtime_ready_init {
            return Err(AppError::new(
                AppErrorCode::Io,
                format!("目标磁盘可用空间仅剩 {free_gb:.1} GB，不足以容纳扩展运行时与权重（建议预留 >= 15 GB）"),
            ));
        }
    }

    let clients = vec![
        downloader::build_direct_http_client()?,
        downloader::build_proxy_fallback_client()?,
    ];

    fs::write(
        ext_path.join("s2a_worker.py"),
        worker_template::S2A_WORKER_PY.as_bytes(),
    )?;
    fs::write(
        ext_path.join("s2a_rebuild_psd.py"),
        worker_template::S2A_REBUILD_PSD_PY.as_bytes(),
    )?;

    if !repo_ready_init {
        emit_progress("repo", 3.0, 0.0, 20.0, 0.0, "正在下载 see-through 推理核心源码...");
        let zip_path = ext_path.join("see-through-main.zip");
        let repo_urls = vec![
            "https://ghfast.top/https://github.com/shitagaki-lab/see-through/archive/refs/heads/main.zip".to_string(),
            "https://github.com/shitagaki-lab/see-through/archive/refs/heads/main.zip".to_string(),
        ];
        let acc = Arc::new(AtomicU64::new(0));
        downloader::download_file_parallel(
            &clients,
            &repo_urls,
            &zip_path,
            None,
            4,
            &cancel_flag,
            &acc,
            &|| {},
        )
        .await?;

        let st_dir = ext_path.join("see-through");
        let tmp_extract = ext_path.join("_repo_extract_tmp");
        let _ = fs::remove_dir_all(&tmp_extract);
        fs::create_dir_all(&tmp_extract)?;

        let f = fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(f)
            .map_err(|e| AppError::new(AppErrorCode::Io, format!("Failed to open repo zip: {e}")))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
            let Some(rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let dest = tmp_extract.join(rel);
            if entry.is_dir() {
                let _ = fs::create_dir_all(&dest);
            } else {
                if let Some(p) = dest.parent() {
                    let _ = fs::create_dir_all(p);
                }
                let mut out = fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        let _ = fs::remove_file(&zip_path);
        let inner = tmp_extract.join("see-through-main");
        if st_dir.exists() {
            let _ = fs::remove_dir_all(&st_dir);
        }
        if inner.exists() {
            fs::rename(&inner, &st_dir)?;
        }
        let _ = fs::remove_dir_all(&tmp_extract);

        let common_assets = st_dir.join("common").join("assets");
        let root_assets = st_dir.join("assets");
        if common_assets.exists() && !root_assets.exists() {
            crate::state::copy_dir_public(&common_assets, &root_assets)?;
        }
    }

    if !runtime_ready_init {
        emit_progress(
            "runtime",
            10.0,
            0.0,
            3200.0,
            0.0,
            "正在准备独立 64 位 Python 3.12 与 CUDA PyTorch 运行时（首次安装需几分钟）...",
        );
        let uv_bin = ensure_uv_executable(&ext_path, &cancel_flag).await?;

        let uv_cache = ext_path.join(".uv-cache");
        let uv_python = ext_path.join(".uv-python");
        let venv_dir = ext_path.join(".venv");
        let venv_python = venv_dir.join("Scripts").join("python.exe");
        let st_common = ext_path.join("see-through").join("common");

        let pypi_idx = pypi_mirror
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(if mirror == "huggingface" {
                "https://pypi.org/simple"
            } else {
                "https://pypi.tuna.tsinghua.edu.cn/simple"
            })
            .to_string();

        let torch_idx = if mirror == "huggingface" {
            "https://download.pytorch.org/whl/cu128"
        } else {
            "https://mirrors.aliyun.com/pytorch-wheels/cu128/"
        }
        .to_string();

        let uv_bin_clone = uv_bin.clone();
        let app_clone = app.clone();
        let ext_c2 = ext_path.clone();
        tokio::task::spawn_blocking(move || -> AppResult<()> {
            let run_uv = |args: &[&str], err_ctx: &str| -> AppResult<()> {
                let mut cmd = Command::new(&uv_bin_clone);
                cmd.args(args)
                    .current_dir(&ext_c2)
                    .env("UV_CACHE_DIR", &uv_cache)
                    .env("UV_PYTHON_INSTALL_DIR", &uv_python);
                #[cfg(target_os = "windows")]
                cmd.creation_flags(0x08000000);
                let out = cmd.output().map_err(|e| {
                    AppError::new(AppErrorCode::Io, format!("{err_ctx}: failed to spawn uv: {e}"))
                })?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(AppError::new(
                        AppErrorCode::Io,
                        format!("{err_ctx}: {}", stderr.chars().take(600).collect::<String>()),
                    ));
                }
                Ok(())
            };

            let _ = app_clone.emit(
                "decompose-install-progress",
                InstallProgressPayload {
                    stage: "runtime".to_string(),
                    percent: 15.0,
                    downloaded_mb: 0.0,
                    total_mb: 3500.0,
                    speed_mb_s: 0.0,
                    message: "正在安装独立 Python 3.12.13 (x64) 并创建隔离虚拟环境...".to_string(),
                },
            );
            run_uv(
                &["venv", &venv_dir.to_string_lossy(), "--python", "3.12", "--clear"],
                "Create isolated Python 3.12 venv failed",
            )?;

            let _ = app_clone.emit(
                "decompose-install-progress",
                InstallProgressPayload {
                    stage: "runtime".to_string(),
                    percent: 22.0,
                    downloaded_mb: 0.0,
                    total_mb: 3500.0,
                    speed_mb_s: 0.0,
                    message: "正在安装 PyTorch 2.8.0 (CUDA 12.8)...".to_string(),
                },
            );
            let py_str = venv_python.to_string_lossy().to_string();
            let torch_res = run_uv(
                &[
                    "pip",
                    "install",
                    "--python",
                    &py_str,
                    "torch==2.8.0+cu128",
                    "torchvision==0.23.0+cu128",
                    "--index-url",
                    &torch_idx,
                ],
                "Install PyTorch cu128",
            );
            if torch_res.is_err() && torch_idx != "https://download.pytorch.org/whl/cu128" {
                // Automatic fallback to official PyTorch wheel CDN
                run_uv(
                    &[
                        "pip",
                        "install",
                        "--python",
                        &py_str,
                        "torch==2.8.0+cu128",
                        "torchvision==0.23.0+cu128",
                        "--index-url",
                        "https://download.pytorch.org/whl/cu128",
                    ],
                    "Install PyTorch cu128 (official fallback)",
                )?;
            } else {
                torch_res?;
            }

            let _ = app_clone.emit(
                "decompose-install-progress",
                InstallProgressPayload {
                    stage: "runtime".to_string(),
                    percent: 38.0,
                    downloaded_mb: 0.0,
                    total_mb: 3500.0,
                    speed_mb_s: 0.0,
                    message: "正在安装 Diffusers / BitsAndBytes / PSD-Tools 推理组件...".to_string(),
                },
            );

            let req_txt = ext_c2.join("s2a_requirements.txt");
            fs::write(&req_txt, worker_template::S2A_PIP_PACKAGES.join("\n"))?;
            run_uv(
                &[
                    "pip",
                    "install",
                    "--python",
                    &py_str,
                    "-r",
                    &req_txt.to_string_lossy(),
                    "--index-url",
                    &pypi_idx,
                ],
                "Install inference packages failed",
            )?;

            run_uv(
                &[
                    "pip",
                    "install",
                    "--python",
                    &py_str,
                    "--no-deps",
                    "-e",
                    &st_common.to_string_lossy(),
                ],
                "Install see-through common module failed",
            )?;

            Ok(())
        })
        .await
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))??;
    }

    // Stage 3: Rust 8-way parallel Range download of NF4/BF16 model weights
    let (ld_sub, mg_sub, ld_repo, mg_repo) = model_subdir_names(&quant_mode);
    let repos = vec![
        (ld_repo, ext_path.join("models").join(ld_sub)),
        (mg_repo, ext_path.join("models").join(mg_sub)),
    ];

    emit_progress("models", 45.0, 0.0, 5400.0, 0.0, "正在解析模型权重文件列表...");
    let mut download_plan: Vec<(String, PathBuf, Option<u64>, Vec<String>)> = Vec::new();
    let mut total_bytes: u64 = 0;

    for (repo_id, local_dir) in repos {
        let siblings = downloader::fetch_hf_repo_files(&clients, repo_id, &mirror).await?;
        for s in siblings {
            let Some(rel_path) = downloader::sanitize_hf_relative_path(&s.rfilename) else {
                return Err(AppError::new(
                    AppErrorCode::InvalidInput,
                    format!("Unsafe model file path rejected: {}", s.rfilename),
                ));
            };
            let sz = s.size.unwrap_or(0);
            total_bytes += sz;
            let dest = local_dir.join(rel_path);
            let urls: Vec<String> = downloader::ordered_endpoints(&mirror)
                .into_iter()
                .map(|ep| format!("{ep}/{repo_id}/resolve/main/{}", s.rfilename))
                .collect();
            download_plan.push((s.rfilename, dest, s.size, urls));
        }
    }

    let progress_acc = Arc::new(AtomicU64::new(0));
    let started_at = Instant::now();
    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);

    for (rel_name, dest_path, expected_sz, urls) in download_plan {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(AppError::new(AppErrorCode::Cancelled, "安装已取消"));
        }
        let app_c = app.clone();
        let acc_c = progress_acc.clone();
        let name_c = rel_name.clone();
        let on_tick = move || {
            let dl = acc_c.load(Ordering::Relaxed);
            let dl_mb = dl as f64 / (1024.0 * 1024.0);
            let elapsed = started_at.elapsed().as_secs_f64().max(0.2);
            let speed_mb_s = dl_mb / elapsed;
            let ratio = if total_bytes > 0 {
                (dl as f64 / total_bytes as f64).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let pct = 45.0 + ratio * 55.0;
            let _ = app_c.emit(
                "decompose-install-progress",
                InstallProgressPayload {
                    stage: "models".to_string(),
                    percent: (pct * 10.0).round() / 10.0,
                    downloaded_mb: (dl_mb * 10.0).round() / 10.0,
                    total_mb: (total_mb * 10.0).round() / 10.0,
                    speed_mb_s: (speed_mb_s * 100.0).round() / 100.0,
                    message: format!("多线程加速拉取权重：{name_c}"),
                },
            );
        };

        downloader::download_file_parallel(
            &clients,
            &urls,
            &dest_path,
            expected_sz,
            8,
            &cancel_flag,
            &progress_acc,
            &on_tick,
        )
        .await?;
    }

    emit_progress("done", 100.0, total_mb, total_mb, 0.0, "拆图工坊独立扩展已准备就绪！");
    Ok(get_decompose_env_status(&state))
}

pub fn safe_uninstall_extension_dir(target: &Path) -> AppResult<()> {
    if !target.exists() {
        return Ok(());
    }
    const MANAGED_DIRS: &[&str] = &[
        ".venv",
        ".uv-cache",
        ".uv-python",
        "models",
        "see-through",
        "tools",
        "see-through-extract-tmp",
    ];
    const MANAGED_FILES: &[&str] = &[
        "s2a_worker.py",
        "s2a_rebuild_psd.py",
        "s2a_split_layer.py",
        "s2a_requirements.txt",
        "see-through-main.zip",
    ];

    for dir_name in MANAGED_DIRS {
        let p = target.join(dir_name);
        if p.exists() {
            fs::remove_dir_all(&p).map_err(|e| {
                AppError::new(
                    AppErrorCode::Io,
                    format!("清理扩展子目录失败 ({}): {e}", p.display()),
                )
            })?;
        }
    }
    for file_name in MANAGED_FILES {
        let p = target.join(file_name);
        if p.exists() {
            fs::remove_file(&p).map_err(|e| {
                AppError::new(
                    AppErrorCode::Io,
                    format!("清理扩展脚本失败 ({}): {e}", p.display()),
                )
            })?;
        }
    }

    // Only delete target itself if now completely empty; unrelated user files remain untouched.
    let _ = fs::remove_dir(target);
    Ok(())
}

pub fn uninstall_decompose_extension(state: &AppState) -> AppResult<DecomposeEnvStatus> {
    let status = get_decompose_env_status(state);
    let target = PathBuf::from(&status.effective_dir);
    safe_uninstall_extension_dir(&target)?;
    Ok(get_decompose_env_status(state))
}

fn image_looks_opaque_background(img: &image::DynamicImage) -> bool {
    let (w, h) = img.dimensions();
    if w < 16 || h < 16 {
        return false;
    }
    let rgba = img.to_rgba8();
    let step_x = (w / 64).max(1);
    let step_y = (h / 64).max(1);
    let mut sampled: u64 = 0;
    let mut transparent: u64 = 0;
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            sampled += 1;
            if rgba.get_pixel(x, y)[3] < 240 {
                transparent += 1;
            }
            x += step_x;
        }
        y += step_y;
    }
    if sampled > 0 && (transparent * 20) >= sampled {
        return false;
    }

    let sample_pts = [
        (2, 2),
        (w - 3, 2),
        (2, h - 3),
        (w - 3, h - 3),
        (w / 2, 2),
    ];
    let opaque_corners = sample_pts
        .iter()
        .filter(|&&(x, y)| rgba.get_pixel(x, y)[3] >= 245)
        .count();
    opaque_corners >= 4
}

fn flood_remove_border_bg(img: &image::DynamicImage) -> RgbaImage {
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 4 || h < 4 {
        return rgba;
    }
    let bg = *rgba.get_pixel(1, 1);
    let mut visited = vec![false; (w * h) as usize];
    let mut q = std::collections::VecDeque::new();
    for x in 0..w {
        q.push_back((x, 0));
        q.push_back((x, h - 1));
    }
    for y in 0..h {
        q.push_back((0, y));
        q.push_back((w - 1, y));
    }
    while let Some((x, y)) = q.pop_front() {
        let idx = (y * w + x) as usize;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        let px = *rgba.get_pixel(x, y);
        let dist = (px[0] as i32 - bg[0] as i32).abs()
            + (px[1] as i32 - bg[1] as i32).abs()
            + (px[2] as i32 - bg[2] as i32).abs();
        if dist <= 45 {
            rgba.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            if x > 0 {
                q.push_back((x - 1, y));
            }
            if x + 1 < w {
                q.push_back((x + 1, y));
            }
            if y > 0 {
                q.push_back((x, y - 1));
            }
            if y + 1 < h {
                q.push_back((x, y + 1));
            }
        }
    }
    rgba
}

fn normalize_dirty_transparent_pixels(img: &image::DynamicImage, job_dir: &Path) -> AppResult<Option<PathBuf>> {
    let mut rgba = img.to_rgba8();
    let has_dirty = rgba
        .pixels()
        .any(|px| px[3] == 0 && (px[0] < 250 || px[1] < 250 || px[2] < 250));
    if !has_dirty {
        return Ok(None);
    }
    for px in rgba.pixels_mut() {
        if px[3] == 0 {
            px[0] = 255;
            px[1] = 255;
            px[2] = 255;
        }
    }
    let pre_path = job_dir.join("prepared_rgba_input.png");
    rgba.save_with_format(&pre_path, ImageFormat::Png)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    Ok(Some(pre_path))
}

pub fn prepare_transparent_input_if_needed(
    state: &AppState,
    input_path: &Path,
    job_dir: &Path,
    auto_remove_bg: bool,
) -> AppResult<PathBuf> {
    let img = image::open(input_path)
        .map_err(|e| AppError::new(AppErrorCode::Io, format!("无法读取输入立绘: {e}")))?;

    if !auto_remove_bg || !image_looks_opaque_background(&img) {
        if let Some(clean_path) = normalize_dirty_transparent_pixels(&img, job_dir)? {
            return Ok(clean_path);
        }
        return Ok(input_path.to_path_buf());
    }

    let models = state.matting.list_models();
    let best_model = models
        .iter()
        .find(|m| m.installed && m.id == "birefnet")
        .or_else(|| models.iter().find(|m| m.installed && m.id == "isnet-anime"))
        .or_else(|| models.iter().find(|m| m.installed));

    let mut rgba: RgbaImage = if let Some(m) = best_model {
        state
            .matting
            .run_matting(&img, &m.id, "auto", false, 0)
            .map(|(out, _)| out)
            .unwrap_or_else(|_| flood_remove_border_bg(&img))
    } else {
        flood_remove_border_bg(&img)
    };

    // Normalize fully transparent pixels to clean white RGB so See-Through does not ingest dark fringe noise
    for px in rgba.pixels_mut() {
        if px[3] == 0 {
            px[0] = 255;
            px[1] = 255;
            px[2] = 255;
        }
    }

    let pre_path = job_dir.join("prepared_rgba_input.png");
    rgba.save_with_format(&pre_path, ImageFormat::Png)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    Ok(pre_path)
}

pub fn run_decompose_blocking(
    app: Option<&tauri::AppHandle>,
    state: &AppState,
    opts: RunDecomposeOptions,
) -> AppResult<DecomposeJobResult> {
    use tauri::Emitter;
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let _guard = decompose_mutex()
        .lock()
        .map_err(|_| AppError::new(AppErrorCode::Io, "Decompose lock poisoned"))?;

    let status = get_decompose_env_status(state);
    if !status.ready {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "拆图扩展尚未安装就绪，请先在「拆图工坊」配置目录并完成扩展与权重下载",
        ));
    }

    let src_path = PathBuf::from(opts.input_path.trim());
    if !src_path.is_file() {
        return Err(AppError::new(
            AppErrorCode::NotFound,
            format!("输入图像不存在: {}", src_path.display()),
        ));
    }

    let settings = state.settings.lock().unwrap().clone();
    let project = opts
        .project
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(settings.active_project.as_str())
        .to_string();

    let ext_dir = PathBuf::from(&status.effective_dir);
    let worker_py = ext_dir.join("s2a_worker.py");
    fs::write(&worker_py, worker_template::S2A_WORKER_PY.as_bytes())?;
    fs::write(
        ext_dir.join("s2a_rebuild_psd.py"),
        worker_template::S2A_REBUILD_PSD_PY.as_bytes(),
    )?;
    fs::write(
        ext_dir.join("s2a_split_layer.py"),
        worker_template::S2A_SPLIT_LAYER_PY.as_bytes(),
    )?;

    let run_cancel = run_cancel_flag();
    run_cancel.store(false, Ordering::Relaxed);

    let job_id = format!(
        "decompose_{}_{}",
        chrono::Local::now().format("%Y%m%d_%H%M%S"),
        &uuid::Uuid::new_v4().simple().to_string()[..6]
    );
    let project_dir = if project.is_empty() {
        state.library_root.clone()
    } else {
        state.library_root.join(&project)
    };
    let job_dir = project_dir.join(&job_id);
    fs::create_dir_all(&job_dir)?;

    state.update_decompose_tracker_progress("prepare", 1, "正在检查立绘透明底并准备推理目录...");
    if let Some(app_h) = app {
        let _ = app_h.emit(
            "decompose-run-progress",
            RunProgressPayload {
                stage: "prepare".to_string(),
                percent: 1,
                message: "正在检查立绘透明底并准备推理目录...".to_string(),
            },
        );
    }

    let mut effective_src_path = src_path.clone();
    if src_path.starts_with(std::env::temp_dir()) {
        let archived_src = job_dir.join("source_input.png");
        if fs::copy(&src_path, &archived_src).is_ok() {
            effective_src_path = archived_src;
        }
    }

    let auto_bg = opts.auto_remove_bg.unwrap_or(true);
    let prepared_src = prepare_transparent_input_if_needed(state, &effective_src_path, &job_dir, auto_bg)?;
    // Explicitly reclaim DirectML matting VRAM before starting CUDA Sidecar
    state.matting.unload_session();

    let quant_mode = opts
        .quant_mode
        .as_deref()
        .unwrap_or(status.quant_mode.as_str());
    let worker_quant = if quant_mode == "bf16" || quant_mode == "none" {
        "none"
    } else {
        "nf4"
    };
    let (ld_sub, mg_sub, _, _) = model_subdir_names(quant_mode);
    let model_layerdiff = ext_dir.join("models").join(ld_sub);
    let model_depth = ext_dir.join("models").join(mg_sub);
    let python_exe = ext_dir.join(".venv").join("Scripts").join("python.exe");
    let repo_root = ext_dir.join("see-through");

    let resolution = opts.resolution.unwrap_or(1024).clamp(768, 1536);
    let steps = opts.steps.unwrap_or(20).clamp(10, 50);
    let seed = opts.seed.unwrap_or(42);
    let tblr_split = opts.tblr_split.unwrap_or(true);
    let cpu_offload = opts
        .cpu_offload
        .unwrap_or(status.vram_total_mb > 0 && status.vram_total_mb < 7800);
    let group_offload = match opts.group_offload.as_deref() {
        Some("on") => "on",
        Some("off") => "off",
        _ => "auto",
    };
    let cool_ms = opts.cool_ms.unwrap_or(60).min(500);

    let mut cmd = Command::new(&python_exe);
    cmd.args([
        worker_py.to_string_lossy().as_ref(),
        "--repo_root",
        repo_root.to_string_lossy().as_ref(),
        "--srcp",
        prepared_src.to_string_lossy().as_ref(),
        "--out_dir",
        job_dir.to_string_lossy().as_ref(),
        "--model_layerdiff",
        model_layerdiff.to_string_lossy().as_ref(),
        "--model_depth",
        model_depth.to_string_lossy().as_ref(),
        "--quant_mode",
        worker_quant,
        "--resolution",
        &resolution.to_string(),
        "--steps",
        &steps.to_string(),
        "--seed",
        &seed.to_string(),
        "--group_offload",
        group_offload,
        "--cool_ms",
        &cool_ms.to_string(),
    ]);
    if tblr_split {
        cmd.arg("--tblr_split");
    }
    if cpu_offload {
        cmd.arg("--cpu_offload");
    }
    cmd.env("HF_HUB_OFFLINE", "1")
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::new(
            AppErrorCode::Io,
            format!("无法启动拆图 Sidecar 进程: {e}"),
        )
    })?;
    if let Ok(mut pid_guard) = active_run_pid_lock().lock() {
        *pid_guard = Some(child.id());
    }
    let _pid_guard = ActivePidGuard;

    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::new(AppErrorCode::Io, "Failed to capture worker stdout")
    })?;
    let mut stderr = child.stderr.take();
    let stderr_handle = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(ref mut err) = stderr {
            let _ = err.read_to_string(&mut s);
        }
        s
    });

    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        if let Some(json_str) = line.strip_prefix("S2A_EVENT ") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                let stage = v
                    .get("stage")
                    .and_then(|x| x.as_str())
                    .unwrap_or("running")
                    .to_string();
                let percent = v.get("percent").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let message = v
                    .get("message")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                state.update_decompose_tracker_progress(&stage, percent, &message);
                if let Some(app_h) = app {
                    let _ = app_h.emit(
                        "decompose-run-progress",
                        RunProgressPayload {
                            stage,
                            percent,
                            message,
                        },
                    );
                }
            }
        }
    }

    let exit = child.wait().map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    let stderr_text = stderr_handle.join().unwrap_or_default();
    if run_cancel.load(Ordering::Relaxed) {
        return Err(AppError::new(
            AppErrorCode::Cancelled,
            "已中止本次拆图推理并释放显卡显存",
        ));
    }
    if !exit.success() {
        let tail: String = stderr_text
            .lines()
            .rev()
            .take(15)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(AppError::new(
            AppErrorCode::MattingFailed,
            format!("拆图推理进程异常退出 ({exit}):\n{tail}"),
        ));
    }

    let manifest_path = job_dir.join("manifest.json");
    let raw_manifest = fs::read_to_string(&manifest_path).map_err(|e| {
        AppError::new(
            AppErrorCode::Io,
            format!("未能读取拆图清单 manifest.json: {e}"),
        )
    })?;
    let manifest: DecomposeManifest = serde_json::from_str(&raw_manifest).map_err(|e| {
        AppError::new(
            AppErrorCode::Io,
            format!("解析 manifest.json 失败: {e}"),
        )
    })?;
    let clean_source_path = effective_src_path.to_string_lossy().replace('\\', "/");
    if let Ok(mut manifest_val) = serde_json::from_str::<serde_json::Value>(&raw_manifest) {
        if let Some(obj) = manifest_val.as_object_mut() {
            obj.insert("project".to_string(), serde_json::json!(project));
            obj.insert("sourcePath".to_string(), serde_json::json!(clean_source_path));
            if let Ok(updated_raw) = serde_json::to_string_pretty(&manifest_val) {
                let _ = fs::write(&manifest_path, updated_raw);
            }
        }
    }

    // Archive reconstruction PNG in library.db so cross-tab filmstrips & MCP stay synced
    let asset_id = if Path::new(&manifest.reconstruction_path).is_file() {
        let prompt_label = format!(
            "Decompose [{} layers · {}px]",
            manifest.layers.len(),
            resolution
        );
        let new_id = uuid::Uuid::new_v4().to_string();
        let item = crate::library::db::AssetItem {
            id: new_id.clone(),
            project: project.clone(),
            archetype: "character_card".to_string(),
            file_path: manifest.reconstruction_path.clone(),
            prompt: prompt_label,
            model: format!("see-through-{quant_mode}"),
            via: "see-through".to_string(),
            width: manifest.width,
            height: manifest.height,
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        state.db.insert_asset(&item).ok().map(|_| new_id)
    } else {
        None
    };

    Ok(DecomposeJobResult {
        job_id,
        job_dir: job_dir.to_string_lossy().replace('\\', "/"),
        project,
        source_path: clean_source_path,
        manifest_path: manifest_path.to_string_lossy().replace('\\', "/"),
        psd_path: manifest.psd_path,
        reconstruction_path: manifest.reconstruction_path,
        width: manifest.width,
        height: manifest.height,
        peak_vram_gb: manifest.peak_vram_gb,
        elapsed_sec: manifest.elapsed_sec,
        layers: manifest.layers,
        asset_id,
    })
}

fn infer_project_from_job_dir(job_dir: &Path) -> String {
    let Some(parent) = job_dir.parent() else {
        return String::new();
    };
    let Some(parent_name) = parent.file_name().and_then(|s| s.to_str()) else {
        return String::new();
    };
    if parent_name.eq_ignore_ascii_case("outputs") {
        return String::new();
    }
    if let Some(grand) = parent.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
        if grand.eq_ignore_ascii_case("outputs") {
            return parent_name.to_string();
        }
    }
    String::new()
}

pub fn load_decompose_job_from_manifest(manifest_path: &Path) -> AppResult<DecomposeJobResult> {
    let raw = fs::read_to_string(manifest_path).map_err(|e| {
        AppError::new(AppErrorCode::Io, format!("无法读取拆图工程文件: {e}"))
    })?;
    let manifest: DecomposeManifest = serde_json::from_str(&raw).map_err(|e| {
        AppError::new(AppErrorCode::Io, format!("拆图工程格式错误: {e}"))
    })?;
    let job_dir = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let job_id = job_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("decompose")
        .to_string();
    let project = if !manifest.project.trim().is_empty() {
        manifest.project
    } else {
        infer_project_from_job_dir(&job_dir)
    };
    let source_path = if !manifest.source_path.trim().is_empty() {
        manifest.source_path
    } else {
        ["source_input.png", "input_transparent.png", "prepared_rgba_input.png"]
            .into_iter()
            .map(|name| job_dir.join(name))
            .find(|p| p.is_file())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default()
    };

    Ok(DecomposeJobResult {
        job_id,
        job_dir: job_dir.to_string_lossy().replace('\\', "/"),
        project,
        source_path,
        manifest_path: manifest_path.to_string_lossy().replace('\\', "/"),
        psd_path: manifest.psd_path,
        reconstruction_path: manifest.reconstruction_path,
        width: manifest.width,
        height: manifest.height,
        peak_vram_gb: manifest.peak_vram_gb,
        elapsed_sec: manifest.elapsed_sec,
        layers: manifest.layers,
        asset_id: None,
    })
}

pub fn list_decompose_history(state: &AppState) -> Vec<DecomposeHistoryItem> {
    let mut items = Vec::new();
    let mut scan_dirs = vec![state.library_root.clone()];
    if let Ok(entries) = fs::read_dir(&state.library_root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && !e.file_name().to_string_lossy().starts_with("decompose_") {
                scan_dirs.push(p);
            }
        }
    }
    for dir in scan_dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && e.file_name().to_string_lossy().starts_with("decompose_") {
                    let mp = p.join("manifest.json");
                    if let Ok(raw) = fs::read_to_string(&mp) {
                        if let Ok(m) = serde_json::from_str::<DecomposeManifest>(&raw) {
                            let ts = fs::metadata(&mp)
                                .and_then(|md| md.modified())
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            items.push(DecomposeHistoryItem {
                                job_id: e.file_name().to_string_lossy().to_string(),
                                job_dir: p.to_string_lossy().replace('\\', "/"),
                                manifest_path: mp.to_string_lossy().replace('\\', "/"),
                                psd_path: m.psd_path,
                                reconstruction_path: m.reconstruction_path,
                                width: m.width,
                                height: m.height,
                                layer_count: m.layers.len(),
                                created_ts: ts,
                            });
                        }
                    }
                }
            }
        }
    }
    items.sort_by(|a, b| b.created_ts.cmp(&a.created_ts));
    items
}

pub fn save_and_rebuild_psd(
    state: &AppState,
    manifest_path: &str,
    layers: Vec<DecomposeLayerInfo>,
    out_psd_path: Option<String>,
) -> AppResult<String> {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mp = PathBuf::from(manifest_path.trim());
    let raw = fs::read_to_string(&mp)?;
    let mut manifest: DecomposeManifest = serde_json::from_str(&raw)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    manifest.layers = layers;
    fs::write(&mp, serde_json::to_string_pretty(&manifest).unwrap())?;

    let status = get_decompose_env_status(state);
    let ext_dir = PathBuf::from(&status.effective_dir);
    let py = ext_dir.join(".venv").join("Scripts").join("python.exe");
    if !py.is_file() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "未检测到拆图 Python 运行时，无法重编 PSD 文件",
        ));
    }

    let rebuild_py = ext_dir.join("s2a_rebuild_psd.py");
    fs::write(&rebuild_py, worker_template::S2A_REBUILD_PSD_PY.as_bytes())?;

    let target_psd = out_psd_path
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| manifest.psd_path.clone());

    let mut cmd = Command::new(&py);
    cmd.args([
        rebuild_py.to_string_lossy().as_ref(),
        "--manifest",
        mp.to_string_lossy().as_ref(),
        "--out_psd",
        &target_psd,
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let out = cmd.output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::new(
            AppErrorCode::Io,
            format!("PSD 重编失败: {err}"),
        ));
    }

    Ok(target_psd.replace('\\', "/"))
}

fn shift_rgba_image(src: &RgbaImage, ox: i32, oy: i32) -> RgbaImage {
    if ox == 0 && oy == 0 {
        return src.clone();
    }
    let (w, h) = src.dimensions();
    let mut dst = RgbaImage::new(w, h);
    let wi = w as i32;
    let hi = h as i32;
    for y in 0..hi {
        let ny = y + oy;
        if ny < 0 || ny >= hi {
            continue;
        }
        for x in 0..wi {
            let nx = x + ox;
            if nx < 0 || nx >= wi {
                continue;
            }
            let px = src.get_pixel(x as u32, y as u32);
            if px[3] > 0 {
                dst.put_pixel(nx as u32, ny as u32, *px);
            }
        }
    }
    dst
}

pub fn save_decompose_layers_state(
    manifest_path: &str,
    layers: Vec<DecomposeLayerInfo>,
) -> AppResult<()> {
    let mp = PathBuf::from(manifest_path.trim());
    let raw = fs::read_to_string(&mp).map_err(|e| {
        AppError::new(
            AppErrorCode::Io,
            format!("无法读取拆图清单 {}: {e}", mp.display()),
        )
    })?;
    let mut manifest: DecomposeManifest = serde_json::from_str(&raw)
        .map_err(|e| AppError::new(AppErrorCode::Io, format!("解析拆图清单失败: {e}")))?;
    manifest.layers = layers;
    fs::write(&mp, serde_json::to_string_pretty(&manifest).unwrap())?;
    Ok(())
}

pub fn export_decompose_parts_bundle(
    manifest_path: &str,
    dest_dir: &str,
    layers: Vec<DecomposeLayerInfo>,
) -> AppResult<String> {
    let mp = PathBuf::from(manifest_path.trim());
    let raw = fs::read_to_string(&mp)?;
    let mut manifest: DecomposeManifest = serde_json::from_str(&raw)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    manifest.layers = layers;

    let out_dir = PathBuf::from(dest_dir.trim());
    fs::create_dir_all(&out_dir)?;

    let mut meta_layers = Vec::new();
    let mut sorted = manifest.layers.clone();
    sorted.sort_by_key(|l| l.z_index);

    for layer in &sorted {
        if !layer.visible {
            continue;
        }
        let src = PathBuf::from(&layer.image_path);
        if !src.is_file() {
            continue;
        }
        let fname = src
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("layer.png")
            .to_string();
        let dest_png = out_dir.join(&fname);
        if layer.offset_x != 0 || layer.offset_y != 0 {
            let dyn_img =
                image::open(&src).map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
            let shifted = shift_rgba_image(&dyn_img.to_rgba8(), layer.offset_x, layer.offset_y);
            shifted
                .save_with_format(&dest_png, ImageFormat::Png)
                .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
        } else {
            fs::copy(&src, &dest_png)?;
        }
        meta_layers.push(serde_json::json!({
            "name": layer.name,
            "file": fname,
            "zIndex": layer.z_index,
            "opacity": layer.opacity,
            "offset": [layer.offset_x, layer.offset_y],
            "bbox": [
                layer.left + layer.offset_x,
                layer.top + layer.offset_y,
                layer.right + layer.offset_x,
                layer.bottom + layer.offset_y
            ],
        }));
    }

    let summary = serde_json::json!({
        "format": "sketch2asset-decompose-v1",
        "width": manifest.width,
        "height": manifest.height,
        "layers": meta_layers,
    });
    fs::write(
        out_dir.join("layers.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )?;

    Ok(out_dir.to_string_lossy().replace('\\', "/"))
}

pub fn apply_layer_defringe(
    layer_image_path: &str,
    choke_px: u32,
) -> AppResult<String> {
    let p = PathBuf::from(layer_image_path.trim());
    if !p.is_file() {
        return Err(AppError::new(
            AppErrorCode::NotFound,
            format!("图层文件不存在: {}", p.display()),
        ));
    }
    let dyn_img = image::open(&p).map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    let rgba = dyn_img.to_rgba8();
    let matte: Vec<f32> = rgba.pixels().map(|px| px[3] as f32 / 255.0).collect();
    let out = matting::compose_rgba(&dyn_img, &matte, true, (choke_px.min(5)) as u8);
    out.save_with_format(&p, ImageFormat::Png)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    Ok(p.to_string_lossy().replace('\\', "/"))
}

pub fn split_decompose_layer(
    state: &AppState,
    manifest_path: &str,
    layers: Vec<DecomposeLayerInfo>,
    layer_id: &str,
    mode: &str,
) -> AppResult<DecomposeJobResult> {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let clean_mode = if mode.trim() == "depth" { "depth" } else { "lr" };
    let mp = PathBuf::from(manifest_path.trim());
    if !mp.is_file() {
        return Err(AppError::new(
            AppErrorCode::NotFound,
            format!("拆图清单不存在: {}", mp.display()),
        ));
    }

    // Persist current visibility, order, opacity and X/Y offsets before splitting
    save_decompose_layers_state(manifest_path, layers)?;

    let status = get_decompose_env_status(state);
    let ext_dir = PathBuf::from(&status.effective_dir);
    let py = ext_dir.join(".venv").join("Scripts").join("python.exe");
    if !py.is_file() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "未检测到拆图 Python 运行时，无法执行二次拆层",
        ));
    }

    let split_py = ext_dir.join("s2a_split_layer.py");
    fs::write(&split_py, worker_template::S2A_SPLIT_LAYER_PY.as_bytes())?;

    let mut cmd = Command::new(&py);
    cmd.args([
        split_py.to_string_lossy().as_ref(),
        "--manifest",
        mp.to_string_lossy().as_ref(),
        "--layer_id",
        layer_id.trim(),
        "--mode",
        clean_mode,
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    let out = cmd.output().map_err(|e| {
        AppError::new(AppErrorCode::Io, format!("启动单层拆分进程失败: {e}"))
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let err_msg = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("单层拆分失败");
        return Err(AppError::new(
            AppErrorCode::MattingFailed,
            err_msg.trim_start_matches("S2A_SPLIT_ERR:").trim().to_string(),
        ));
    }

    load_decompose_job_from_manifest(&mp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_dir_readiness_empty_dir() {
        let tmp = std::env::temp_dir().join(format!(
            "s2a_decompose_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&tmp).unwrap();
        assert_eq!(inspect_dir_readiness(&tmp, "nf4"), (false, false, false));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_image_looks_opaque_vs_transparent() {
        let opaque = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            32,
            32,
            image::Rgba([255, 255, 255, 255]),
        ));
        assert!(image_looks_opaque_background(&opaque));

        let transparent = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            32,
            32,
            image::Rgba([0, 0, 0, 0]),
        ));
        assert!(!image_looks_opaque_background(&transparent));

        // Corners opaque, but significant interior transparency -> must detect as already transparent
        let mut mixed = RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        for y in 16..48 {
            for x in 16..48 {
                mixed.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            }
        }
        assert!(!image_looks_opaque_background(&image::DynamicImage::ImageRgba8(mixed)));
    }

    #[test]
    fn test_model_subdir_mapping() {
        assert_eq!(model_subdir_names("nf4").0, "layerdiff3d_nf4");
        assert_eq!(model_subdir_names("bf16").0, "layerdiff3d");
    }

    #[test]
    fn test_manifest_backwards_compat_and_new_fields() {
        let legacy_json = r#"{
            "width": 1024,
            "height": 1024,
            "psdPath": "a.psd",
            "reconstructionPath": "r.png",
            "peakVramGb": 6.5,
            "elapsedSec": 45.2,
            "layers": []
        }"#;
        let parsed: DecomposeManifest = serde_json::from_str(legacy_json).unwrap();
        assert!(!parsed.face_recovered);
        assert!(!parsed.used_group_offload);

        let new_json = r#"{
            "width": 1024,
            "height": 1024,
            "psdPath": "a.psd",
            "reconstructionPath": "r.png",
            "peakVramGb": 6.5,
            "elapsedSec": 45.2,
            "faceRecovered": true,
            "usedGroupOffload": true,
            "layers": []
        }"#;
        let parsed_new: DecomposeManifest = serde_json::from_str(new_json).unwrap();
        assert!(parsed_new.face_recovered);
        assert!(parsed_new.used_group_offload);
    }

    #[test]
    fn test_layer_offset_defaults_and_shift_rgba() {
        let legacy_layer = r#"{
            "id": "layer_01_face",
            "name": "face",
            "zIndex": 1,
            "visible": true,
            "opacity": 1.0,
            "left": 10,
            "top": 20,
            "right": 30,
            "bottom": 40,
            "imagePath": "face.png"
        }"#;
        let info: DecomposeLayerInfo = serde_json::from_str(legacy_layer).unwrap();
        assert_eq!(info.offset_x, 0);
        assert_eq!(info.offset_y, 0);

        let mut src = RgbaImage::new(8, 8);
        src.put_pixel(2, 3, image::Rgba([200, 100, 50, 255]));
        let shifted = shift_rgba_image(&src, 3, -1);
        assert_eq!(shifted.get_pixel(2, 3)[3], 0);
        assert_eq!(*shifted.get_pixel(5, 2), image::Rgba([200, 100, 50, 255]));
    }

    #[test]
    fn test_safe_uninstall_preserves_unrelated_user_files() {
        let root = std::env::temp_dir().join(format!("s2a_uninst_{}", uuid::Uuid::new_v4()));
        let ext_sub = root.join(".venv").join("Scripts");
        let models_sub = root.join("models");
        let user_proj = root.join("my_game_project");
        fs::create_dir_all(&ext_sub).unwrap();
        fs::create_dir_all(&models_sub).unwrap();
        fs::create_dir_all(&user_proj).unwrap();
        fs::write(ext_sub.join("python.exe"), b"stub").unwrap();
        fs::write(models_sub.join("weights.bin"), b"stub").unwrap();
        fs::write(root.join("s2a_worker.py"), b"print(1)").unwrap();
        fs::write(user_proj.join("hero.png"), b"keep-me").unwrap();

        safe_uninstall_extension_dir(&root).unwrap();
        assert!(!root.join(".venv").exists());
        assert!(!root.join("models").exists());
        assert!(!root.join("s2a_worker.py").exists());
        // Unrelated user files & folder must stay intact
        assert!(user_proj.join("hero.png").is_file());
        let _ = fs::remove_dir_all(&root);

        // Clean extension-only dir is removed completely when empty
        let clean_root = std::env::temp_dir().join(format!("s2a_uninst_clean_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(clean_root.join(".venv")).unwrap();
        fs::write(clean_root.join("s2a_worker.py"), b"x").unwrap();
        safe_uninstall_extension_dir(&clean_root).unwrap();
        assert!(!clean_root.exists());
    }

    #[test]
    fn test_load_decompose_job_recovers_project_from_manifest_and_parent_dir() {
        let base = std::env::temp_dir().join(format!("s2a_proj_{}", uuid::Uuid::new_v4()));
        let job_dir = base.join("outputs").join("SideScrollerRPG").join("decompose_20261125_120000_abc123");
        fs::create_dir_all(&job_dir).unwrap();
        let legacy_manifest = job_dir.join("manifest.json");
        fs::write(
            &legacy_manifest,
            r#"{
                "width": 1024,
                "height": 1024,
                "psdPath": "a.psd",
                "reconstructionPath": "r.png",
                "peakVramGb": 5.0,
                "elapsedSec": 60.0,
                "layers": []
            }"#,
        )
        .unwrap();
        fs::write(job_dir.join("source_input.png"), b"png").unwrap();

        let loaded = load_decompose_job_from_manifest(&legacy_manifest).unwrap();
        assert_eq!(loaded.project, "SideScrollerRPG");
        assert!(loaded.source_path.ends_with("source_input.png"));

        let _ = fs::remove_dir_all(&base);
    }
}
