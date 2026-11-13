pub mod comfyui;

use base64::Engine;
use reqwest::multipart;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{AppError, AppErrorCode, AppResult};

const DEFAULT_MAX_ATTEMPTS: usize = 3;
const DEFAULT_RETRY_DELAYS_MS: [u64; 2] = [500, 1200];
const DEFAULT_USER_AGENT: &str = concat!("Sketch2Asset/", env!("CARGO_PKG_VERSION"));
static BACKOFF_SEQ: AtomicU64 = AtomicU64::new(1);

struct HttpResponseData {
    status: reqwest::StatusCode,
    bytes: Vec<u8>,
    attempts: usize,
}

impl HttpResponseData {
    fn text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

pub(crate) fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut cur = err.source();
    while let Some(src) = cur {
        let s = src.to_string();
        if !parts.last().map(|p| p == &s).unwrap_or(false) {
            parts.push(s);
        }
        cur = src.source();
    }
    parts.join(": ")
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 502 | 503 | 504)
}

fn is_retryable_transport_error(err: &reqwest::Error) -> bool {
    if err.is_builder() || err.is_redirect() {
        return false;
    }
    if err.is_connect() || err.is_timeout() || err.is_request() {
        return true;
    }
    let chain = format_error_chain(err).to_ascii_lowercase();
    chain.contains("incomplete message")
        || chain.contains("unexpected eof")
        || chain.contains("connection reset")
        || chain.contains("connection closed")
        || chain.contains("broken pipe")
        || chain.contains("timed out")
        || chain.contains("channel closed")
}

fn next_jitter_u64() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let seq = BACKOFF_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut x = nanos ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x
}

fn compute_retry_delay_ms(
    retry_index: usize,
    base_delays_ms: &[u64],
    retry_after_header: Option<&str>,
    jitter_raw: u64,
) -> u64 {
    let base = base_delays_ms
        .get(retry_index)
        .copied()
        .unwrap_or_else(|| base_delays_ms.last().copied().unwrap_or(1200));
    if base == 0 {
        return 0;
    }
    let jitter_span = (base / 4).max(1);
    let jittered = base.saturating_sub(jitter_span / 2) + (jitter_raw % jitter_span);
    if let Some(raw) = retry_after_header {
        if let Ok(secs) = raw.trim().parse::<u64>() {
            if secs > 0 {
                return jittered.max(secs.saturating_mul(1000).min(5000));
            }
        }
    }
    jittered
}

fn map_transport_error(
    endpoint: &str,
    url: &str,
    attempt: usize,
    max_attempts: usize,
    err: &reqwest::Error,
) -> AppError {
    let chain = format_error_chain(err);
    let kind = if err.is_timeout() {
        "timeout"
    } else if err.is_connect() {
        "connect"
    } else if err.is_request() {
        "request/send"
    } else if err.is_body() || err.is_decode() {
        "body/stream"
    } else {
        "transport"
    };
    let code = if err.is_timeout() || err.is_connect() {
        AppErrorCode::Network
    } else {
        AppErrorCode::ProviderUnavailable
    };
    AppError::with_details(
        code,
        format!(
            "HTTP {} {} failure for {} (attempt {}/{}): {}",
            endpoint, kind, url, attempt, max_attempts, chain
        ),
        chain.clone(),
    )
}

fn extract_upstream_error_message(raw_body: &str) -> String {
    let mut msg = raw_body.trim().to_string();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw_body) {
        if let Some(err) = val.get("error") {
            if let Some(m) = err.get("message").and_then(|v| v.as_str()) {
                msg = m.to_string();
            } else if let Some(s) = err.as_str() {
                msg = s.to_string();
            }
        } else if let Some(m) = val.get("message").and_then(|v| v.as_str()) {
            msg = m.to_string();
        }
    }
    msg
}

fn map_upstream_error_with_endpoint(
    endpoint: &str,
    attempt: usize,
    max_attempts: usize,
    status: reqwest::StatusCode,
    raw_body: &str,
) -> AppError {
    let msg = extract_upstream_error_message(raw_body);
    let code = match status.as_u16() {
        401 | 403 => AppErrorCode::ProviderAuth,
        429 => AppErrorCode::ProviderRateLimited,
        _ => AppErrorCode::ProviderUnavailable,
    };
    AppError::with_details(
        code,
        format!(
            "{} HTTP {} (attempt {}/{}): {}",
            endpoint,
            status.as_u16(),
            attempt,
            max_attempts,
            msg
        ),
        raw_body,
    )
}

async fn send_with_retry_config<F>(
    endpoint: &str,
    url: &str,
    max_attempts: usize,
    retry_delays_ms: &[u64],
    retry_body_errors: bool,
    mut build_req: F,
) -> AppResult<HttpResponseData>
where
    F: FnMut() -> AppResult<reqwest::RequestBuilder>,
{
    let attempts_cap = max_attempts.max(1);
    for attempt in 1..=attempts_cap {
        let req = build_req()?.header(reqwest::header::USER_AGENT, DEFAULT_USER_AGENT);
        match req.send().await {
            Err(err) => {
                let chain = format_error_chain(&err);
                if attempt < attempts_cap && is_retryable_transport_error(&err) {
                    let delay = compute_retry_delay_ms(
                        attempt - 1,
                        retry_delays_ms,
                        None,
                        next_jitter_u64(),
                    );
                    eprintln!(
                        "[providers] {} transient send error (attempt {}/{}), retrying in {}ms: {}",
                        endpoint, attempt, attempts_cap, delay, chain
                    );
                    if delay > 0 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                    continue;
                }
                eprintln!(
                    "[providers] {} send failed (attempt {}/{}): {}",
                    endpoint, attempt, attempts_cap, chain
                );
                return Err(map_transport_error(
                    endpoint,
                    url,
                    attempt,
                    attempts_cap,
                    &err,
                ));
            }
            Ok(resp) => {
                let status = resp.status();
                let retry_after = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                if is_retryable_status(status) && attempt < attempts_cap {
                    let delay = compute_retry_delay_ms(
                        attempt - 1,
                        retry_delays_ms,
                        retry_after.as_deref(),
                        next_jitter_u64(),
                    );
                    eprintln!(
                        "[providers] {} HTTP {} (attempt {}/{}), retrying in {}ms",
                        endpoint,
                        status.as_u16(),
                        attempt,
                        attempts_cap,
                        delay
                    );
                    drop(resp);
                    if delay > 0 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                    continue;
                }

                match resp.bytes().await {
                    Ok(bytes) => {
                        return Ok(HttpResponseData {
                            status,
                            bytes: bytes.to_vec(),
                            attempts: attempt,
                        });
                    }
                    Err(err) => {
                        let chain = format_error_chain(&err);
                        if retry_body_errors
                            && attempt < attempts_cap
                            && is_retryable_transport_error(&err)
                        {
                            let delay = compute_retry_delay_ms(
                                attempt - 1,
                                retry_delays_ms,
                                None,
                                next_jitter_u64(),
                            );
                            eprintln!(
                                "[providers] {} body read error (attempt {}/{}), retrying in {}ms: {}",
                                endpoint, attempt, attempts_cap, delay, chain
                            );
                            if delay > 0 {
                                tokio::time::sleep(Duration::from_millis(delay)).await;
                            }
                            continue;
                        }
                        if !status.is_success() {
                            return Err(map_upstream_error_with_endpoint(
                                endpoint,
                                attempt,
                                attempts_cap,
                                status,
                                &format!("response body read error: {}", chain),
                            ));
                        }
                        eprintln!(
                            "[providers] {} body read failed after HTTP {} (attempt {}/{}): {}",
                            endpoint,
                            status.as_u16(),
                            attempt,
                            attempts_cap,
                            chain
                        );
                        return Err(map_transport_error(
                            endpoint,
                            url,
                            attempt,
                            attempts_cap,
                            &err,
                        ));
                    }
                }
            }
        }
    }

    unreachable!("retry loop always returns before exhausting attempts")
}

async fn send_with_retry<F>(
    endpoint: &str,
    url: &str,
    retry_body_errors: bool,
    build_req: F,
) -> AppResult<HttpResponseData>
where
    F: FnMut() -> AppResult<reqwest::RequestBuilder>,
{
    send_with_retry_config(
        endpoint,
        url,
        DEFAULT_MAX_ATTEMPTS,
        &DEFAULT_RETRY_DELAYS_MS,
        retry_body_errors,
        build_req,
    )
    .await
}

pub fn normalize_base_url(raw: &str) -> AppResult<String> {
    let mut u = raw.trim().trim_end_matches('/').to_string();
    if u.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "API Base URL is empty",
        ));
    }
    if !u.starts_with("http://") && !u.starts_with("https://") {
        u = format!("https://{}", u);
    }
    for suffix in ["/chat/completions", "/images/generations", "/images/edits", "/models"] {
        if u.ends_with(suffix) {
            u.truncate(u.len() - suffix.len());
            u = u.trim_end_matches('/').to_string();
        }
    }
    if let Ok(parsed) = reqwest::Url::parse(&u) {
        if parsed.path() == "/" || parsed.path().is_empty() {
            u = format!("{}/v1", u);
        }
    }
    Ok(u)
}

pub fn map_upstream_error(status: reqwest::StatusCode, raw_body: &str) -> AppError {
    let msg = extract_upstream_error_message(raw_body);
    let code = match status.as_u16() {
        401 | 403 => AppErrorCode::ProviderAuth,
        429 => AppErrorCode::ProviderRateLimited,
        _ => AppErrorCode::ProviderUnavailable,
    };
    AppError::with_details(
        code,
        format!("HTTP {}: {}", status.as_u16(), msg),
        raw_body,
    )
}

pub async fn fetch_models(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
) -> AppResult<Vec<String>> {
    let base = normalize_base_url(api_base)?;
    if api_key.trim().is_empty() {
        return Err(AppError::new(
            AppErrorCode::MissingApiKey,
            "API Key is required",
        ));
    }
    let url = format!("{}/models", base);
    let resp = send_with_retry("models", &url, true, || {
        Ok(client
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .bearer_auth(api_key.trim()))
    })
    .await?;
    let text = resp.text_lossy();
    if !resp.status.is_success() {
        return Err(map_upstream_error_with_endpoint(
            "models",
            resp.attempts,
            DEFAULT_MAX_ATTEMPTS,
            resp.status,
            &text,
        ));
    }

    let val: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        AppError::with_details(
            AppErrorCode::ProviderBadResponse,
            "Upstream /models did not return valid JSON",
            &text,
        )
    })?;

    let items = if let Some(arr) = val.get("data").and_then(|v| v.as_array()) {
        Some(arr)
    } else if let Some(arr) = val.get("models").and_then(|v| v.as_array()) {
        Some(arr)
    } else {
        val.as_array()
    };

    let mut ids = Vec::new();
    if let Some(list) = items {
        for item in list {
            if let Some(s) = item.as_str() {
                ids.push(s.to_string());
            } else if let Some(obj) = item.as_object() {
                if let Some(id) = obj
                    .get("id")
                    .or_else(|| obj.get("model"))
                    .or_else(|| obj.get("name"))
                    .and_then(|v| v.as_str())
                {
                    ids.push(id.to_string());
                }
            }
        }
    }

    ids.sort_by_key(|a| a.to_lowercase());
    ids.dedup();

    let img_re = regex::Regex::new(
        r"(?i)image|img|dall|flux|seedream|seededit|banana|imagen|stable|sdxl|sd3|midjourney|\bmj\b|kolors|cogview|wanx|hunyuan-image|ideogram|recraft|playground|pixart|photon|irag",
    )
    .unwrap();

    let (mut img_models, mut other_models): (Vec<String>, Vec<String>) =
        ids.into_iter().partition(|m| img_re.is_match(m));
    img_models.append(&mut other_models);

    if img_models.is_empty() {
        return Err(AppError::with_details(
            AppErrorCode::ProviderBadResponse,
            "No models returned by upstream /models",
            &text,
        ));
    }
    Ok(img_models)
}

pub fn parse_size_dims(size: &str) -> Option<(u32, u32)> {
    let trimmed = size.trim().to_lowercase();
    if trimmed.is_empty() || trimmed == "auto" {
        return None;
    }
    let parts: Vec<&str> = trimmed.split(['x', '×', '*']).collect();
    if parts.len() != 2 {
        return None;
    }
    let w = parts[0].trim().parse::<u32>().ok()?;
    let h = parts[1].trim().parse::<u32>().ok()?;
    if w >= 64 && h >= 64 && w <= 8192 && h <= 8192 {
        Some((w, h))
    } else {
        None
    }
}

pub fn fallback_aspect_bucket(size: &str) -> &'static str {
    if let Some((w, h)) = parse_size_dims(size) {
        let ratio = w as f64 / h as f64;
        if ratio > 1.15 {
            "1536x1024"
        } else if ratio < 0.87 {
            "1024x1536"
        } else {
            "1024x1024"
        }
    } else {
        "1024x1024"
    }
}

fn build_multipart_edit_form(
    model: &str,
    prompt: &str,
    size: &str,
    image_bytes: &[u8],
    image_mime: &str,
    image_filename: &str,
    mask_bytes: Option<(&[u8], &str)>,
) -> AppResult<multipart::Form> {
    let img_part = multipart::Part::bytes(image_bytes.to_vec())
        .file_name(image_filename.to_string())
        .mime_str(image_mime)
        .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;
    let mut form = multipart::Form::new()
        .text("model", model.to_string())
        .text("prompt", prompt.to_string())
        .part("image", img_part);
    if let Some((m_bytes, m_mime)) = mask_bytes {
        let mask_part = multipart::Part::bytes(m_bytes.to_vec())
            .file_name("mask.png")
            .mime_str(m_mime)
            .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;
        form = form.part("mask", mask_part);
    }
    if !size.is_empty() && size != "auto" {
        form = form.text("size", size.to_string());
    }
    Ok(form)
}

async fn execute_images_request_with_size_fallback<F>(
    client: &reqwest::Client,
    endpoint: &str,
    url: &str,
    size: &str,
    max_attempts: usize,
    retry_delays_ms: &[u64],
    mut build_for_size: F,
) -> AppResult<(Vec<u8>, String)>
where
    F: FnMut(&str) -> AppResult<reqwest::RequestBuilder>,
{
    let mut resp = send_with_retry_config(
        endpoint,
        url,
        max_attempts,
        retry_delays_ms,
        false,
        || build_for_size(size),
    )
    .await?;

    // If upstream model rejected a non-standard size (e.g. 1920x1080 or 3840x2160 on OpenAI gpt-image-1),
    // automatically retry with the closest standard aspect-ratio bucket (1536x1024 / 1024x1536 / 1024x1024).
    if resp.status == reqwest::StatusCode::BAD_REQUEST && !size.is_empty() && size != "auto" {
        let fb = fallback_aspect_bucket(size);
        if fb != size {
            resp = send_with_retry_config(
                endpoint,
                url,
                max_attempts,
                retry_delays_ms,
                false,
                || build_for_size(fb),
            )
            .await?;
        }
    }

    let text = resp.text_lossy();
    if !resp.status.is_success() {
        return Err(map_upstream_error_with_endpoint(
            endpoint,
            resp.attempts,
            max_attempts,
            resp.status,
            &text,
        ));
    }

    let bytes = extract_image_bytes_from_images_resp(client, &text).await?;
    Ok((bytes, endpoint.to_string()))
}

async fn generate_via_images_inner(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    size: &str,
    sketch_bytes: Option<(Vec<u8>, String)>,
    max_attempts: usize,
    retry_delays_ms: &[u64],
) -> AppResult<(Vec<u8>, String)> {
    let base = normalize_base_url(api_base)?;
    if let Some((bytes, mime)) = sketch_bytes {
        let ext = match mime.as_str() {
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => "png",
        };
        let filename = format!("sketch.{}", ext);
        let url = format!("{}/images/edits", base);

        return execute_images_request_with_size_fallback(
            client,
            "images/edits",
            &url,
            size,
            max_attempts,
            retry_delays_ms,
            |sz| {
                let form = build_multipart_edit_form(
                    model, prompt, sz, &bytes, &mime, &filename, None,
                )?;
                Ok(client
                    .post(&url)
                    .header(reqwest::header::ACCEPT, "application/json")
                    .bearer_auth(api_key.trim())
                    .multipart(form))
            },
        )
        .await;
    }

    let url = format!("{}/images/generations", base);
    execute_images_request_with_size_fallback(
        client,
        "images/generations",
        &url,
        size,
        max_attempts,
        retry_delays_ms,
        |sz| {
            let mut payload = serde_json::json!({
                "model": model,
                "prompt": prompt,
            });
            if !sz.is_empty() && sz != "auto" {
                payload["size"] = serde_json::Value::String(sz.to_string());
            }
            Ok(client
                .post(&url)
                .header(reqwest::header::ACCEPT, "application/json")
                .bearer_auth(api_key.trim())
                .json(&payload))
        },
    )
    .await
}

pub async fn generate_via_images(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    size: &str,
    sketch_bytes: Option<(Vec<u8>, String)>,
) -> AppResult<(Vec<u8>, String)> {
    generate_via_images_inner(
        client,
        api_base,
        api_key,
        model,
        prompt,
        size,
        sketch_bytes,
        DEFAULT_MAX_ATTEMPTS,
        &DEFAULT_RETRY_DELAYS_MS,
    )
    .await
}

async fn extract_image_bytes_from_images_resp(
    client: &reqwest::Client,
    text: &str,
) -> AppResult<Vec<u8>> {
    let val: serde_json::Value = serde_json::from_str(text).map_err(|_| {
        AppError::with_details(
            AppErrorCode::ProviderBadResponse,
            "Upstream did not return valid JSON",
            text,
        )
    })?;
    let item = val
        .get("data")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| {
            AppError::with_details(
                AppErrorCode::ProviderBadResponse,
                "Response missing data[0]",
                text,
            )
        })?;

    if let Some(b64) = item.get("b64_json").and_then(|v| v.as_str()) {
        return decode_base64_image(b64);
    }
    if let Some(url) = item.get("url").and_then(|v| v.as_str()) {
        return download_image_bytes(client, url).await;
    }
    Err(AppError::with_details(
        AppErrorCode::ProviderBadResponse,
        "Response missing b64_json or url",
        text,
    ))
}

async fn send_chat_completions_with_modalities_fallback(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    body: &mut serde_json::Value,
) -> AppResult<HttpResponseData> {
    let resp = {
        let body_ref: &serde_json::Value = body;
        send_with_retry("chat/completions", url, false, || {
            Ok(client
                .post(url)
                .header(reqwest::header::ACCEPT, "application/json")
                .bearer_auth(api_key.trim())
                .json(body_ref))
        })
        .await?
    };

    if resp.status == reqwest::StatusCode::BAD_REQUEST {
        if let Some(obj) = body.as_object_mut() {
            obj.remove("modalities");
        }
        let body_ref: &serde_json::Value = body;
        return send_with_retry("chat/completions", url, false, || {
            Ok(client
                .post(url)
                .header(reqwest::header::ACCEPT, "application/json")
                .bearer_auth(api_key.trim())
                .json(body_ref))
        })
        .await;
    }

    Ok(resp)
}

pub async fn generate_via_chat(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    sketch_bytes: Option<(Vec<u8>, String)>,
) -> AppResult<(Vec<u8>, String)> {
    let base = normalize_base_url(api_base)?;
    let mut parts = vec![serde_json::json!({
        "type": "text",
        "text": prompt
    })];

    if let Some((bytes, mime)) = sketch_bytes {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let data_url = format!("data:{};base64,{}", mime, b64);
        parts.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": data_url }
        }));
    }

    let mut body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": parts }],
        "modalities": ["image", "text"]
    });

    let url = format!("{}/chat/completions", base);
    let resp = send_chat_completions_with_modalities_fallback(client, &url, api_key, &mut body).await?;
    let text = resp.text_lossy();
    if !resp.status.is_success() {
        return Err(map_upstream_error_with_endpoint(
            "chat/completions",
            resp.attempts,
            DEFAULT_MAX_ATTEMPTS,
            resp.status,
            &text,
        ));
    }

    let val: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        AppError::with_details(
            AppErrorCode::ProviderBadResponse,
            "Upstream chat response is not valid JSON",
            &text,
        )
    })?;

    if let Some(bytes) = extract_image_from_chat_json(client, &val).await {
        return Ok((bytes, "chat/completions".to_string()));
    }

    Err(AppError::with_details(
        AppErrorCode::ProviderBadResponse,
        "Chat completion did not contain an image output",
        &text,
    ))
}

pub async fn edit_regions_via_chat(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    original_bytes: (Vec<u8>, String),
    guide_bytes: Option<(Vec<u8>, String)>,
) -> AppResult<(Vec<u8>, String)> {
    let base = normalize_base_url(api_base)?;
    let orig_b64 = base64::engine::general_purpose::STANDARD.encode(&original_bytes.0);
    let orig_data_url = format!("data:{};base64,{}", original_bytes.1, orig_b64);

    let mut parts = vec![
        serde_json::json!({
            "type": "text",
            "text": prompt
        }),
        serde_json::json!({
            "type": "image_url",
            "image_url": { "url": orig_data_url }
        }),
    ];

    if let Some((g_bytes, g_mime)) = &guide_bytes {
        let g_b64 = base64::engine::general_purpose::STANDARD.encode(g_bytes);
        let g_data_url = format!("data:{};base64,{}", g_mime, g_b64);
        parts.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": g_data_url }
        }));
    }

    let mut body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": parts }],
        "modalities": ["image", "text"]
    });

    let url = format!("{}/chat/completions", base);
    let resp = send_chat_completions_with_modalities_fallback(client, &url, api_key, &mut body).await?;

    // If upstream does not support 2 images in one message, fallback to single visual guide image
    if !resp.status.is_success() && !matches!(resp.status.as_u16(), 401 | 403) {
        if let Some((g_bytes, g_mime)) = guide_bytes {
            return generate_via_chat(client, api_base, api_key, model, prompt, Some((g_bytes, g_mime))).await;
        }
    }

    let text = resp.text_lossy();
    if !resp.status.is_success() {
        return Err(map_upstream_error_with_endpoint(
            "chat/completions",
            resp.attempts,
            DEFAULT_MAX_ATTEMPTS,
            resp.status,
            &text,
        ));
    }

    let val: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        AppError::with_details(
            AppErrorCode::ProviderBadResponse,
            "Upstream chat response is not valid JSON",
            &text,
        )
    })?;

    if let Some(bytes) = extract_image_from_chat_json(client, &val).await {
        return Ok((bytes, "chat/completions (region-guide)".to_string()));
    }

    Err(AppError::with_details(
        AppErrorCode::ProviderBadResponse,
        "Chat completion did not contain an image output",
        &text,
    ))
}

pub async fn edit_regions_via_images(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    size: &str,
    original_bytes: (Vec<u8>, String),
    guide_bytes: Option<(Vec<u8>, String)>,
    mask_bytes: Option<(Vec<u8>, String)>,
) -> AppResult<(Vec<u8>, String)> {
    let base = normalize_base_url(api_base)?;
    let url = format!("{}/images/edits", base);

    // 1. Try standard /images/edits with image + mask if mask_bytes is provided
    if let Some((m_bytes, m_mime)) = mask_bytes {
        match send_with_retry("images/edits", &url, false, || {
            let form = build_multipart_edit_form(
                model,
                prompt,
                size,
                &original_bytes.0,
                &original_bytes.1,
                "image.png",
                Some((&m_bytes, &m_mime)),
            )?;
            Ok(client
                .post(&url)
                .header(reqwest::header::ACCEPT, "application/json")
                .bearer_auth(api_key.trim())
                .multipart(form))
        })
        .await
        {
            Ok(resp) => {
                let text = resp.text_lossy();
                if resp.status.is_success() {
                    let bytes = extract_image_bytes_from_images_resp(client, &text).await?;
                    return Ok((bytes, "images/edits (mask)".to_string()));
                }
                if matches!(resp.status.as_u16(), 401 | 403) {
                    return Err(map_upstream_error_with_endpoint(
                        "images/edits",
                        resp.attempts,
                        DEFAULT_MAX_ATTEMPTS,
                        resp.status,
                        &text,
                    ));
                }
            }
            Err(err) => {
                eprintln!("[providers] region mask edit fallback after error: {}", err.message);
            }
        }
    }

    // 2. Try /images/edits with visual guide image (or original image)
    let target_img = guide_bytes.clone().unwrap_or_else(|| original_bytes.clone());
    match send_with_retry("images/edits", &url, false, || {
        let form = build_multipart_edit_form(
            model,
            prompt,
            size,
            &target_img.0,
            &target_img.1,
            "image.png",
            None,
        )?;
        Ok(client
            .post(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .bearer_auth(api_key.trim())
            .multipart(form))
    })
    .await
    {
        Ok(resp) => {
            let text = resp.text_lossy();
            if resp.status.is_success() {
                let bytes = extract_image_bytes_from_images_resp(client, &text).await?;
                return Ok((bytes, "images/edits (guide)".to_string()));
            }
            if matches!(resp.status.as_u16(), 401 | 403) {
                return Err(map_upstream_error_with_endpoint(
                    "images/edits",
                    resp.attempts,
                    DEFAULT_MAX_ATTEMPTS,
                    resp.status,
                    &text,
                ));
            }
        }
        Err(err) => {
            eprintln!("[providers] region guide edit fallback to chat after error: {}", err.message);
        }
    }

    // 3. Fallback to multimodal chat endpoint
    edit_regions_via_chat(client, api_base, api_key, model, prompt, original_bytes, guide_bytes).await
}

async fn extract_image_from_chat_json(
    client: &reqwest::Client,
    val: &serde_json::Value,
) -> Option<Vec<u8>> {
    let msg = val.get("choices")?.as_array()?.first()?.get("message")?;

    if let Some(images) = msg.get("images").and_then(|v| v.as_array()) {
        for im in images {
            let candidate = im
                .get("image_url")
                .and_then(|iu| iu.get("url"))
                .and_then(|v| v.as_str())
                .or_else(|| im.get("url").and_then(|v| v.as_str()))
                .or_else(|| im.get("b64_json").and_then(|v| v.as_str()))
                .or_else(|| im.as_str());
            if let Some(s) = candidate {
                if let Ok(b) = resolve_url_or_b64(client, s).await {
                    return Some(b);
                }
            }
        }
    }

    let mut text_segments = Vec::new();
    if let Some(arr) = msg.get("content").and_then(|v| v.as_array()) {
        for part in arr {
            let ptype = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(ptype, "image_url" | "image" | "output_image") {
                let candidate = part
                    .get("image_url")
                    .and_then(|iu| iu.get("url"))
                    .and_then(|v| v.as_str())
                    .or_else(|| part.get("url").and_then(|v| v.as_str()));
                if let Some(s) = candidate {
                    if let Ok(b) = resolve_url_or_b64(client, s).await {
                        return Some(b);
                    }
                }
            } else if ptype == "text" {
                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                    text_segments.push(t.to_string());
                }
            }
        }
    } else if let Some(s) = msg.get("content").and_then(|v| v.as_str()) {
        text_segments.push(s.to_string());
    }

    let full_text = text_segments.join("\n");
    if !full_text.is_empty() {
        if let Ok(re_data) =
            regex::Regex::new(r"data:image/[a-zA-Z0-9.+-]+;base64,([A-Za-z0-9+/=]+)")
        {
            if let Some(caps) = re_data.captures(&full_text) {
                if let Some(b64) = caps.get(1) {
                    if let Ok(bytes) = decode_base64_image(b64.as_str()) {
                        return Some(bytes);
                    }
                }
            }
        }
        if let Ok(re_url) = regex::Regex::new(r#"https?://[^\s)\]"'<>]+"#) {
            for m in re_url.find_iter(&full_text) {
                if let Ok(bytes) = download_image_bytes(client, m.as_str()).await {
                    return Some(bytes);
                }
            }
        }
    }
    None
}

async fn resolve_url_or_b64(client: &reqwest::Client, s: &str) -> AppResult<Vec<u8>> {
    let trimmed = s.trim();
    if let Some(idx) = trimmed.find(";base64,") {
        return decode_base64_image(&trimmed[idx + 8..]);
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return download_image_bytes(client, trimmed).await;
    }
    decode_base64_image(trimmed)
}

fn decode_base64_image(b64: &str) -> AppResult<Vec<u8>> {
    let cleaned: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(cleaned)
        .map_err(|e| AppError::new(AppErrorCode::ProviderBadResponse, e.to_string()))
}

async fn download_image_bytes(client: &reqwest::Client, url: &str) -> AppResult<Vec<u8>> {
    let resp = send_with_retry("image-download", url, true, || {
        Ok(client
            .get(url)
            .header(reqwest::header::ACCEPT, "image/*,*/*"))
    })
    .await?;
    if !resp.status.is_success() {
        return Err(AppError::new(
            AppErrorCode::ProviderBadResponse,
            format!(
                "Failed to download generated image from {} (HTTP {}, attempt {}/{})",
                url,
                resp.status.as_u16(),
                resp.attempts,
                DEFAULT_MAX_ATTEMPTS
            ),
        ));
    }
    Ok(resp.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    enum MockStep {
        DropImmediately,
        Respond { status: &'static str, body: String },
    }

    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end = None;
        while header_end.is_none() {
            let n = match stream.read(&mut tmp).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = Some(pos + 4);
            }
        }
        if let Some(hend) = header_end {
            let headers_str = String::from_utf8_lossy(&buf[..hend]).to_string();
            let content_len = headers_str
                .lines()
                .find_map(|line| {
                    let (k, v) = line.split_once(':')?;
                    if k.trim().eq_ignore_ascii_case("content-length") {
                        v.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            while buf.len() < hend + content_len {
                let n = match stream.read(&mut tmp).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&tmp[..n]);
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    async fn spawn_mock_server(
        steps: Vec<MockStep>,
    ) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let rec_clone = recorded.clone();
        let handle = tokio::spawn(async move {
            for step in steps {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                match step {
                    MockStep::DropImmediately => {
                        rec_clone.lock().unwrap().push("<DROPPED>".to_string());
                        drop(stream);
                    }
                    MockStep::Respond { status, body } => {
                        let req = read_http_request(&mut stream).await;
                        rec_clone.lock().unwrap().push(req);
                        let resp = format!(
                            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status,
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                        let _ = stream.shutdown().await;
                    }
                }
            }
        });
        (format!("http://{}", addr), recorded, handle)
    }

    #[test]
    fn test_normalize_base_url_variants() {
        assert_eq!(
            normalize_base_url("https://api.freeshengtu.com").unwrap(),
            "https://api.freeshengtu.com/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.freeshengtu.com/v1/").unwrap(),
            "https://api.freeshengtu.com/v1"
        );
        assert_eq!(
            normalize_base_url("api.freeshengtu.com/v1/images/generations/").unwrap(),
            "https://api.freeshengtu.com/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.freeshengtu.com/v1/images/edits").unwrap(),
            "https://api.freeshengtu.com/v1"
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:8080/v1/chat/completions").unwrap(),
            "http://127.0.0.1:8080/v1"
        );
        assert!(normalize_base_url("   ").is_err());
    }

    #[test]
    fn test_compute_retry_delay_ms_jitter_and_retry_after() {
        let d0 = compute_retry_delay_ms(0, &DEFAULT_RETRY_DELAYS_MS, None, 17);
        assert!((438..=562).contains(&d0), "d0={} out of ~500ms band", d0);
        let d1 = compute_retry_delay_ms(1, &DEFAULT_RETRY_DELAYS_MS, None, 99);
        assert!((1050..=1349).contains(&d1), "d1={} out of ~1200ms band", d1);
        let ra = compute_retry_delay_ms(0, &DEFAULT_RETRY_DELAYS_MS, Some("3"), 0);
        assert_eq!(ra, 3000);
        let ra_capped = compute_retry_delay_ms(0, &DEFAULT_RETRY_DELAYS_MS, Some("30"), 0);
        assert_eq!(ra_capped, 5000);
    }

    #[tokio::test]
    async fn test_images_edits_retries_transient_drop_and_503_with_rebuilt_multipart() {
        let expected = b"fake-png-output";
        let b64 = base64::engine::general_purpose::STANDARD.encode(expected);
        let ok_body = serde_json::json!({ "data": [{ "b64_json": b64 }] }).to_string();
        let (base, recorded, srv) = spawn_mock_server(vec![
            MockStep::DropImmediately,
            MockStep::Respond {
                status: "503 Service Unavailable",
                body: r#"{"error":{"message":"upstream busy"}}"#.to_string(),
            },
            MockStep::Respond {
                status: "200 OK",
                body: ok_body,
            },
        ])
        .await;

        let client = reqwest::Client::new();
        let (bytes, via) = generate_via_images_inner(
            &client,
            &base,
            "sk-test-key",
            "gpt-image-1",
            "warrior sprite",
            "1024x1024",
            Some((b"sketch-bytes".to_vec(), "image/png".to_string())),
            3,
            &[5, 10],
        )
        .await
        .expect("should recover on 3rd attempt");

        assert_eq!(via, "images/edits");
        assert_eq!(bytes, expected);
        let reqs = recorded.lock().unwrap();
        assert_eq!(reqs.len(), 3);
        assert_eq!(reqs[0], "<DROPPED>");
        assert!(reqs[1].starts_with("POST /v1/images/edits "));
        assert!(reqs[2].starts_with("POST /v1/images/edits "));
        assert!(reqs[2].contains("sketch-bytes"));
        assert!(reqs[2].contains("sketch.png"));
        srv.abort();
    }

    #[tokio::test]
    async fn test_images_edits_never_silently_falls_back_to_generations() {
        let (base, recorded, srv) = spawn_mock_server(vec![MockStep::Respond {
            status: "404 Not Found",
            body: r#"{"error":{"message":"edits unsupported"}}"#.to_string(),
        }])
        .await;

        let client = reqwest::Client::new();
        let err = generate_via_images_inner(
            &client,
            &base,
            "sk-test-key",
            "gpt-image-1",
            "warrior sprite",
            "1024x1024",
            Some((b"sketch-bytes".to_vec(), "image/png".to_string())),
            3,
            &[5, 10],
        )
        .await
        .expect_err("must fail instead of falling back to /images/generations");

        assert!(err.message.contains("images/edits"), "msg={}", err.message);
        assert!(err.message.contains("404"), "msg={}", err.message);
        assert!(err.message.contains("attempt 1/3"), "msg={}", err.message);

        let reqs = recorded.lock().unwrap();
        assert_eq!(reqs.len(), 1);
        assert!(reqs[0].starts_with("POST /v1/images/edits "));
        srv.abort();
    }

    #[tokio::test]
    async fn test_images_generations_retries_exhausted_reports_endpoint_and_attempts() {
        let (base, recorded, srv) = spawn_mock_server(vec![
            MockStep::DropImmediately,
            MockStep::Respond {
                status: "502 Bad Gateway",
                body: r#"{"error":{"message":"bad gateway 1"}}"#.to_string(),
            },
            MockStep::Respond {
                status: "504 Gateway Timeout",
                body: r#"{"error":{"message":"gateway timeout"}}"#.to_string(),
            },
        ])
        .await;

        let client = reqwest::Client::new();
        let err = generate_via_images_inner(
            &client,
            &base,
            "sk-test-key",
            "gpt-image-1",
            "sword icon",
            "1024x1024",
            None,
            3,
            &[5, 10],
        )
        .await
        .expect_err("3 transient failures must return error");

        assert!(
            err.message.contains("images/generations"),
            "msg={}",
            err.message
        );
        assert!(err.message.contains("504"), "msg={}", err.message);
        assert!(err.message.contains("attempt 3/3"), "msg={}", err.message);
        assert_eq!(recorded.lock().unwrap().len(), 3);
        srv.abort();
    }

    #[tokio::test]
    async fn test_images_generations_400_falls_back_to_aspect_bucket_without_transient_retry() {
        let expected = b"ok-fallback";
        let b64 = base64::engine::general_purpose::STANDARD.encode(expected);
        let ok_body = serde_json::json!({ "data": [{ "b64_json": b64 }] }).to_string();
        let (base, recorded, srv) = spawn_mock_server(vec![
            MockStep::Respond {
                status: "400 Bad Request",
                body: r#"{"error":{"message":"unsupported size 1920x1080"}}"#.to_string(),
            },
            MockStep::Respond {
                status: "200 OK",
                body: ok_body,
            },
        ])
        .await;

        let client = reqwest::Client::new();
        let (bytes, via) = generate_via_images_inner(
            &client,
            &base,
            "sk-test-key",
            "gpt-image-1",
            "banner",
            "1920x1080",
            None,
            3,
            &[5, 10],
        )
        .await
        .expect("aspect bucket retry should succeed");

        assert_eq!(via, "images/generations");
        assert_eq!(bytes, expected);
        let reqs = recorded.lock().unwrap();
        assert_eq!(reqs.len(), 2);
        assert!(reqs[0].contains("1920x1080"));
        assert!(reqs[1].contains("1536x1024"));
        srv.abort();
    }
}
