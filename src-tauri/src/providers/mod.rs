use base64::Engine;
use reqwest::multipart;

use crate::error::{AppError, AppErrorCode, AppResult};

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
    let resp = client
        .get(&url)
        .bearer_auth(api_key.trim())
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(map_upstream_error(status, &text));
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

pub async fn generate_via_images(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    size: &str,
    sketch_bytes: Option<(Vec<u8>, String)>,
) -> AppResult<(Vec<u8>, String)> {
    let base = normalize_base_url(api_base)?;
    if let Some((bytes, mime)) = sketch_bytes {
        let ext = match mime.as_str() {
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => "png",
        };
        let url = format!("{}/images/edits", base);

        let build_edit_form = |sz: &str| -> AppResult<multipart::Form> {
            let file_part = multipart::Part::bytes(bytes.clone())
                .file_name(format!("sketch.{}", ext))
                .mime_str(&mime)
                .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;
            let mut form = multipart::Form::new()
                .text("model", model.to_string())
                .text("prompt", prompt.to_string())
                .part("image", file_part);
            if !sz.is_empty() && sz != "auto" {
                form = form.text("size", sz.to_string());
            }
            Ok(form)
        };

        if let Ok(form) = build_edit_form(size) {
            if let Ok(r) = client
                .post(&url)
                .bearer_auth(api_key.trim())
                .multipart(form)
                .send()
                .await
            {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                if status.is_success() {
                    let out = extract_image_bytes_from_images_resp(client, &text).await?;
                    return Ok((out, "images/edits".to_string()));
                }
                // If custom/2K/4K size was rejected by strict whitelist, retry with standard aspect bucket
                let fb = fallback_aspect_bucket(size);
                if status == reqwest::StatusCode::BAD_REQUEST && fb != size {
                    if let Ok(form_fb) = build_edit_form(fb) {
                        if let Ok(r2) = client
                            .post(&url)
                            .bearer_auth(api_key.trim())
                            .multipart(form_fb)
                            .send()
                            .await
                        {
                            let status2 = r2.status();
                            let text2 = r2.text().await.unwrap_or_default();
                            if status2.is_success() {
                                let out = extract_image_bytes_from_images_resp(client, &text2).await?;
                                return Ok((out, "images/edits".to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut payload = serde_json::json!({
        "model": model,
        "prompt": prompt,
    });
    if !size.is_empty() && size != "auto" {
        payload["size"] = serde_json::Value::String(size.to_string());
    }

    let url = format!("{}/images/generations", base);
    let resp = client
        .post(&url)
        .bearer_auth(api_key.trim())
        .json(&payload)
        .send()
        .await?;
    let mut status = resp.status();
    let mut text = resp.text().await.unwrap_or_default();

    // If upstream model rejected a non-standard size (e.g. 1920x1080 or 3840x2160 on OpenAI gpt-image-1),
    // automatically retry with the closest standard aspect-ratio bucket (1536x1024 / 1024x1536 / 1024x1024).
    if status == reqwest::StatusCode::BAD_REQUEST && !size.is_empty() && size != "auto" {
        let fb = fallback_aspect_bucket(size);
        if fb != size {
            payload["size"] = serde_json::Value::String(fb.to_string());
            let resp2 = client
                .post(&url)
                .bearer_auth(api_key.trim())
                .json(&payload)
                .send()
                .await?;
            status = resp2.status();
            text = resp2.text().await.unwrap_or_default();
        }
    }

    if !status.is_success() {
        return Err(map_upstream_error(status, &text));
    }

    let bytes = extract_image_bytes_from_images_resp(client, &text).await?;
    Ok((bytes, "images/generations".to_string()))
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
    let mut resp = client
        .post(&url)
        .bearer_auth(api_key.trim())
        .json(&body)
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::BAD_REQUEST {
        if let Some(obj) = body.as_object_mut() {
            obj.remove("modalities");
        }
        resp = client
            .post(&url)
            .bearer_auth(api_key.trim())
            .json(&body)
            .send()
            .await?;
    }

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(map_upstream_error(status, &text));
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
    let mut resp = client
        .post(&url)
        .bearer_auth(api_key.trim())
        .json(&body)
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::BAD_REQUEST {
        if let Some(obj) = body.as_object_mut() {
            obj.remove("modalities");
        }
        resp = client
            .post(&url)
            .bearer_auth(api_key.trim())
            .json(&body)
            .send()
            .await?;
    }

    // If upstream does not support 2 images in one message, fallback to single visual guide image
    if !resp.status().is_success() {
        if let Some((g_bytes, g_mime)) = guide_bytes {
            return generate_via_chat(client, api_base, api_key, model, prompt, Some((g_bytes, g_mime))).await;
        }
    }

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(map_upstream_error(status, &text));
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
        let img_part = multipart::Part::bytes(original_bytes.0.clone())
            .file_name("image.png")
            .mime_str(&original_bytes.1)
            .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;
        let mask_part = multipart::Part::bytes(m_bytes)
            .file_name("mask.png")
            .mime_str(&m_mime)
            .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;

        let mut form = multipart::Form::new()
            .text("model", model.to_string())
            .text("prompt", prompt.to_string())
            .part("image", img_part)
            .part("mask", mask_part);

        if !size.is_empty() && size != "auto" {
            form = form.text("size", size.to_string());
        }

        if let Ok(r) = client
            .post(&url)
            .bearer_auth(api_key.trim())
            .multipart(form)
            .send()
            .await
        {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            if status.is_success() {
                let bytes = extract_image_bytes_from_images_resp(client, &text).await?;
                return Ok((bytes, "images/edits (mask)".to_string()));
            }
        }
    }

    // 2. Try /images/edits with visual guide image (or original image)
    let target_img = guide_bytes.clone().unwrap_or_else(|| original_bytes.clone());
    let img_part = multipart::Part::bytes(target_img.0)
        .file_name("image.png")
        .mime_str(&target_img.1)
        .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;

    let mut form = multipart::Form::new()
        .text("model", model.to_string())
        .text("prompt", prompt.to_string())
        .part("image", img_part);

    if !size.is_empty() && size != "auto" {
        form = form.text("size", size.to_string());
    }

    if let Ok(r) = client
        .post(&url)
        .bearer_auth(api_key.trim())
        .multipart(form)
        .send()
        .await
    {
        let status = r.status();
        let text = r.text().await.unwrap_or_default();
        if status.is_success() {
            let bytes = extract_image_bytes_from_images_resp(client, &text).await?;
            return Ok((bytes, "images/edits (guide)".to_string()));
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
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::new(
            AppErrorCode::ProviderBadResponse,
            format!("Failed to download generated image from {}", url),
        ));
    }
    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}
