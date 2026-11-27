use serde::Deserialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppErrorCode, AppResult};

#[derive(Debug, Clone, Deserialize)]
pub struct HfSibling {
    pub rfilename: String,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct HfModelResponse {
    #[serde(default)]
    siblings: Vec<HfSibling>,
}

pub fn build_direct_http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(90))
        .user_agent("huggingface_hub/1.7.2 Sketch2Asset/0.3.1")
        .build()
        .map_err(|e| AppError::new(AppErrorCode::Network, format!("HTTP client init failed: {e}")))
}

pub fn build_proxy_fallback_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(90))
        .user_agent("huggingface_hub/1.7.2 Sketch2Asset/0.3.1")
        .build()
        .map_err(|e| AppError::new(AppErrorCode::Network, format!("HTTP client init failed: {e}")))
}

pub fn ordered_endpoints(preferred: &str) -> Vec<&'static str> {
    if preferred == "huggingface" {
        vec!["https://huggingface.co", "https://hf-mirror.com"]
    } else {
        vec!["https://hf-mirror.com", "https://huggingface.co"]
    }
}

/// Validate and normalize a HuggingFace repository relative file path (`rfilename`).
/// Rejects path traversal (`..`), drive prefixes, leading slashes, control chars, and `.git` paths.
pub fn sanitize_hf_relative_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(|c| c.is_control())
    {
        return None;
    }

    let mut clean = PathBuf::new();
    for seg in trimmed.split(['/', '\\']) {
        if seg.is_empty() || seg == "." || seg == ".." {
            return None;
        }
        if seg.eq_ignore_ascii_case(".git") && clean.as_os_str().is_empty() {
            return None;
        }
        clean.push(seg);
    }

    if clean.as_os_str().is_empty() {
        return None;
    }
    if !clean
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(clean)
}

pub async fn fetch_hf_repo_files(
    clients: &[reqwest::Client],
    repo_id: &str,
    preferred_mirror: &str,
) -> AppResult<Vec<HfSibling>> {
    let mut last_err = String::new();
    for ep in ordered_endpoints(preferred_mirror) {
        let url = format!("{ep}/api/models/{repo_id}?blobs=true");
        for client in clients {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(parsed) = resp.json::<HfModelResponse>().await {
                        let filtered: Vec<HfSibling> = parsed
                            .siblings
                            .into_iter()
                            .filter(|s| sanitize_hf_relative_path(&s.rfilename).is_some())
                            .collect();
                        if !filtered.is_empty() {
                            return Ok(filtered);
                        }
                    }
                }
                Ok(resp) => {
                    last_err = format!("{url} returned HTTP {}", resp.status());
                }
                Err(e) => {
                    last_err = format!("{url} error: {e}");
                }
            }
        }
    }
    Err(AppError::new(
        AppErrorCode::Network,
        format!("Failed to list HF files for {repo_id}: {last_err}"),
    ))
}

/// Download a single URL with 8 concurrent HTTP Range chunks (for large files >= 8 MB)
/// or single stream (for smaller files / servers not supporting Range).
pub async fn download_file_parallel<F>(
    clients: &[reqwest::Client],
    urls: &[String],
    dest_path: &Path,
    expected_size: Option<u64>,
    concurrency: usize,
    cancel_flag: &Arc<AtomicBool>,
    progress_acc: &Arc<AtomicU64>,
    on_tick: &F,
) -> AppResult<u64>
where
    F: Fn() + Send + Sync,
{
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if let Some(exp) = expected_size {
        if dest_path.exists() {
            if let Ok(meta) = fs::metadata(dest_path) {
                if meta.len() == exp {
                    progress_acc.fetch_add(exp, Ordering::Relaxed);
                    on_tick();
                    return Ok(exp);
                }
            }
        }
    }

    let mut last_err = String::new();
    for url in urls {
        for client in clients {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(AppError::new(AppErrorCode::Cancelled, "Download cancelled by user"));
            }

            let head_res = client.head(url).send().await;
            let (total_size, supports_range) = match head_res {
                Ok(resp) if resp.status().is_success() => {
                    let len = resp
                        .headers()
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .or(expected_size);
                    let accept_ranges = resp
                        .headers()
                        .get(reqwest::header::ACCEPT_RANGES)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.eq_ignore_ascii_case("bytes"))
                        .unwrap_or(false);
                    (len, accept_ranges)
                }
                _ => (expected_size, false),
            };

            let chunk_threshold = 8 * 1024 * 1024;
            if supports_range {
                if let Some(total) = total_size.filter(|&n| n >= chunk_threshold) {
                    match download_via_range_chunks(
                        client,
                        url,
                        dest_path,
                        total,
                        concurrency.clamp(2, 16),
                        cancel_flag,
                        progress_acc,
                        on_tick,
                    )
                    .await
                    {
                        Ok(sz) => return Ok(sz),
                        Err(e) if e.code == AppErrorCode::Cancelled => return Err(e),
                        Err(e) => {
                            last_err = e.message;
                            continue;
                        }
                    }
                }
            }

            // Fallback: single-stream resumable download
            match download_single_stream(
                client,
                url,
                dest_path,
                total_size,
                cancel_flag,
                progress_acc,
                on_tick,
            )
            .await
            {
                Ok(sz) => return Ok(sz),
                Err(e) if e.code == AppErrorCode::Cancelled => return Err(e),
                Err(e) => {
                    last_err = e.message;
                }
            }
        }
    }

    Err(AppError::new(
        AppErrorCode::Network,
        format!("Download failed for {}: {last_err}", dest_path.display()),
    ))
}

async fn download_via_range_chunks<F>(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
    total: u64,
    parts: usize,
    cancel_flag: &Arc<AtomicBool>,
    progress_acc: &Arc<AtomicU64>,
    on_tick: &F,
) -> AppResult<u64>
where
    F: Fn() + Send + Sync,
{
    let part_size = (total + parts as u64 - 1) / parts as u64;
    let mut handles = Vec::with_capacity(parts);

    for idx in 0..parts {
        let start = idx as u64 * part_size;
        if start >= total {
            break;
        }
        let end = ((idx as u64 + 1) * part_size - 1).min(total - 1);
        let expected_part_len = end - start + 1;
        let part_path = PathBuf::from(format!("{}.part.{idx}", dest_path.display()));

        if part_path.exists() {
            if let Ok(meta) = fs::metadata(&part_path) {
                if meta.len() == expected_part_len {
                    progress_acc.fetch_add(expected_part_len, Ordering::Relaxed);
                    on_tick();
                    handles.push(tokio::spawn(async move { Ok::<PathBuf, AppError>(part_path) }));
                    continue;
                }
            }
        }

        let c = client.clone();
        let u = url.to_string();
        let cancel = Arc::clone(cancel_flag);
        let acc = Arc::clone(progress_acc);
        let existing = if part_path.exists() {
            fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        if existing > 0 && existing < expected_part_len {
            acc.fetch_add(existing, Ordering::Relaxed);
        }

        handles.push(tokio::spawn(async move {
            let mut offset = if existing < expected_part_len { existing } else { 0 };
            for attempt in 0..4 {
                if cancel.load(Ordering::Relaxed) {
                    return Err(AppError::new(AppErrorCode::Cancelled, "Cancelled"));
                }
                let req_start = start + offset;
                let range_hdr = format!("bytes={req_start}-{end}");
                let mut resp = match c.get(&u).header(reqwest::header::RANGE, range_hdr).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        if attempt == 3 {
                            return Err(AppError::new(AppErrorCode::Network, e.to_string()));
                        }
                        tokio::time::sleep(Duration::from_secs(1 + attempt as u64)).await;
                        continue;
                    }
                };
                if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT
                    && !(offset == 0 && resp.status().is_success())
                {
                    if attempt == 3 {
                        return Err(AppError::new(
                            AppErrorCode::Network,
                            format!("Range HTTP {}", resp.status()),
                        ));
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                let mut file = if offset > 0 && resp.status() == reqwest::StatusCode::PARTIAL_CONTENT
                {
                    OpenOptions::new().create(true).append(true).open(&part_path)?
                } else {
                    if offset > 0 {
                        acc.fetch_sub(offset, Ordering::Relaxed);
                        offset = 0;
                    }
                    File::create(&part_path)?
                };

                let mut stream_err = None;
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(AppError::new(AppErrorCode::Cancelled, "Cancelled"));
                    }
                    match resp.chunk().await {
                        Ok(Some(bytes)) => {
                            file.write_all(&bytes)?;
                            offset += bytes.len() as u64;
                            acc.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                        }
                        Ok(None) => break,
                        Err(e) => {
                            stream_err = Some(e.to_string());
                            break;
                        }
                    }
                }
                file.flush()?;
                if offset == expected_part_len {
                    return Ok(part_path);
                }
                if attempt == 3 {
                    return Err(AppError::new(
                        AppErrorCode::Network,
                        stream_err.unwrap_or_else(|| {
                            format!("Chunk {idx} incomplete: {offset}/{expected_part_len}")
                        }),
                    ));
                }
            }
            Err(AppError::new(AppErrorCode::Network, "Chunk retry exhausted"))
        }));
    }

    // Periodically tick progress while chunk tasks execute
    let ticker_cancel = Arc::clone(cancel_flag);
    let mut part_files = Vec::with_capacity(handles.len());
    let mut last_emit = Instant::now();
    for handle in handles {
        // Wait for chunk completion while periodically firing UI tick
        let mut h = handle;
        loop {
            if ticker_cancel.load(Ordering::Relaxed) {
                return Err(AppError::new(AppErrorCode::Cancelled, "Cancelled"));
            }
            match tokio::time::timeout(Duration::from_millis(250), &mut h).await {
                Ok(joined) => {
                    let p = joined.map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))??;
                    part_files.push(p);
                    on_tick();
                    break;
                }
                Err(_) => {
                    if last_emit.elapsed() >= Duration::from_millis(250) {
                        on_tick();
                        last_emit = Instant::now();
                    }
                }
            }
        }
    }

    // Merge parts cleanly
    let merged_tmp = PathBuf::from(format!("{}.merging", dest_path.display()));
    {
        let mut out = File::create(&merged_tmp)?;
        let mut buf = vec![0u8; 1024 * 1024];
        for pf in &part_files {
            let mut f = File::open(pf)?;
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
            }
        }
        out.flush()?;
    }

    let merged_len = fs::metadata(&merged_tmp)?.len();
    if merged_len != total {
        let _ = fs::remove_file(&merged_tmp);
        return Err(AppError::new(
            AppErrorCode::Io,
            format!("Merged size mismatch: {merged_len} != {total}"),
        ));
    }

    if dest_path.exists() {
        let _ = fs::remove_file(dest_path);
    }
    fs::rename(&merged_tmp, dest_path)?;
    for pf in part_files {
        let _ = fs::remove_file(pf);
    }
    on_tick();
    Ok(merged_len)
}

async fn download_single_stream<F>(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
    expected_size: Option<u64>,
    cancel_flag: &Arc<AtomicBool>,
    progress_acc: &Arc<AtomicU64>,
    on_tick: &F,
) -> AppResult<u64>
where
    F: Fn() + Send + Sync,
{
    let tmp_path = PathBuf::from(format!("{}.part", dest_path.display()));
    let mut existing = if tmp_path.exists() {
        fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    if existing > 0 {
        progress_acc.fetch_add(existing, Ordering::Relaxed);
    }

    let mut req = client.get(url);
    if existing > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }

    let mut resp = req
        .send()
        .await
        .map_err(|e| AppError::new(AppErrorCode::Network, e.to_string()))?;
    if !resp.status().is_success() && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(AppError::new(
            AppErrorCode::Network,
            format!("HTTP {}", resp.status()),
        ));
    }

    let mut file = if existing > 0 && resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        OpenOptions::new().create(true).append(true).open(&tmp_path)?
    } else {
        if existing > 0 {
            progress_acc.fetch_sub(existing, Ordering::Relaxed);
            existing = 0;
        }
        File::create(&tmp_path)?
    };

    let mut written = existing;
    let mut last_tick = Instant::now();
    while let Some(bytes) = resp
        .chunk()
        .await
        .map_err(|e| AppError::new(AppErrorCode::Network, e.to_string()))?
    {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(AppError::new(AppErrorCode::Cancelled, "Cancelled"));
        }
        file.write_all(&bytes)?;
        written += bytes.len() as u64;
        progress_acc.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        if last_tick.elapsed() >= Duration::from_millis(200) {
            on_tick();
            last_tick = Instant::now();
        }
    }
    file.flush()?;
    drop(file);

    if let Some(exp) = expected_size {
        if written != exp {
            return Err(AppError::new(
                AppErrorCode::Io,
                format!("Size mismatch: {written} != {exp}"),
            ));
        }
    }

    if dest_path.exists() {
        let _ = fs::remove_file(dest_path);
    }
    fs::rename(&tmp_path, dest_path)?;
    on_tick();
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordered_endpoints_priority() {
        assert_eq!(
            ordered_endpoints("hf-mirror"),
            vec!["https://hf-mirror.com", "https://huggingface.co"]
        );
        assert_eq!(
            ordered_endpoints("huggingface"),
            vec!["https://huggingface.co", "https://hf-mirror.com"]
        );
    }

    #[test]
    fn test_sanitize_hf_relative_path_blocks_traversal() {
        assert_eq!(
            sanitize_hf_relative_path("unet/diffusion_pytorch_model.safetensors"),
            Some(PathBuf::from("unet").join("diffusion_pytorch_model.safetensors"))
        );
        assert!(sanitize_hf_relative_path("../../evil.exe").is_none());
        assert!(sanitize_hf_relative_path("sub/..\\escape.bin").is_none());
        assert!(sanitize_hf_relative_path("C:\\Windows\\win.ini").is_none());
        assert!(sanitize_hf_relative_path("/etc/passwd").is_none());
        assert!(sanitize_hf_relative_path(".git/config").is_none());
        assert!(sanitize_hf_relative_path(".gitattributes").is_some());
    }
}
