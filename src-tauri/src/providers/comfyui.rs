use reqwest::multipart;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppErrorCode, AppResult};
use crate::providers::parse_size_dims;

fn default_comfy_mode() -> String {
    "builtin".to_string()
}

fn default_denoise() -> f64 {
    0.72
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComfyWorkflowConfig {
    /// "builtin" | "custom"
    #[serde(default = "default_comfy_mode")]
    pub mode: String,
    /// Display name of imported workflow file
    #[serde(default)]
    pub workflow_name: String,
    /// Raw workflow JSON string (can be UI workflow.json or API workflow_api.json)
    #[serde(default)]
    pub raw_workflow_json: String,
    /// Node ID for positive prompt (e.g. "6"), or empty for auto
    #[serde(default)]
    pub positive_node_id: String,
    #[serde(default)]
    pub positive_field: String,
    /// Node ID for negative prompt (e.g. "7"), or empty for auto/none
    #[serde(default)]
    pub negative_node_id: String,
    #[serde(default)]
    pub negative_field: String,
    /// Node ID for primary reference/sketch image (LoadImage), or empty for auto
    #[serde(default)]
    pub image_node_id: String,
    /// Node ID for secondary reference/mask image (LoadImage), or empty for auto
    #[serde(default)]
    pub secondary_image_node_id: String,
    /// Node ID for latent canvas size (EmptyLatentImage / EmptySD3LatentImage), or empty for auto
    #[serde(default)]
    pub latent_node_id: String,
    /// Node ID for sampler seed (KSampler / RandomNoise), or empty for auto-all
    #[serde(default)]
    pub seed_node_id: String,
    /// Denoise strength for sketch/reference img2img (0.10 - 1.00)
    #[serde(default = "default_denoise")]
    pub denoise_strength: f64,
}

impl Default for ComfyWorkflowConfig {
    fn default() -> Self {
        Self {
            mode: default_comfy_mode(),
            workflow_name: String::new(),
            raw_workflow_json: String::new(),
            positive_node_id: String::new(),
            positive_field: String::new(),
            negative_node_id: String::new(),
            negative_field: String::new(),
            image_node_id: String::new(),
            secondary_image_node_id: String::new(),
            latent_node_id: String::new(),
            seed_node_id: String::new(),
            denoise_strength: default_denoise(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComfyNodeSlotOption {
    pub node_id: String,
    pub class_type: String,
    pub title: String,
    pub field: String,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComfyWorkflowAnalysis {
    pub format: String, // "api" | "ui_converted" | "png_embedded"
    pub node_count: usize,
    pub normalized_api_json: String,
    pub text_nodes: Vec<ComfyNodeSlotOption>,
    pub image_nodes: Vec<ComfyNodeSlotOption>,
    pub latent_nodes: Vec<ComfyNodeSlotOption>,
    pub seed_nodes: Vec<ComfyNodeSlotOption>,
    pub suggested_positive_node_id: String,
    pub suggested_positive_field: String,
    pub suggested_negative_node_id: String,
    pub suggested_negative_field: String,
    pub suggested_image_node_id: String,
    pub suggested_secondary_image_node_id: String,
    pub suggested_latent_node_id: String,
    pub suggested_seed_node_id: String,
}

/// Normalize ComfyUI server root URL (e.g. `http://127.0.0.1:8188`)
pub fn normalize_comfy_url(raw: &str) -> AppResult<String> {
    let mut u = raw.trim().trim_end_matches('/').to_string();
    if u.is_empty() {
        u = "http://127.0.0.1:8188".to_string();
    }
    if !u.starts_with("http://") && !u.starts_with("https://") {
        u = format!("http://{}", u);
    }
    for suffix in ["/v1", "/prompt", "/system_stats", "/object_info"] {
        if u.ends_with(suffix) {
            u.truncate(u.len() - suffix.len());
            u = u.trim_end_matches('/').to_string();
        }
    }
    Ok(u)
}

/// Extract string list from `/object_info` input specification `[[ "modelA.safetensors", "modelB.safetensors" ], ...]`
fn extract_combo_options(object_info: &Value, node_class: &str, param_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(param) = object_info
        .get(node_class)
        .and_then(|n| n.get("input"))
        .and_then(|i| {
            i.get("required")
                .and_then(|r| r.get(param_name))
                .or_else(|| i.get("optional").and_then(|o| o.get(param_name)))
        })
        .and_then(|v| v.as_array())
    else {
        return out;
    };

    if let Some(first) = param.first() {
        if let Some(arr) = first.as_array() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out
}

/// Fetch available local models from ComfyUI (`/object_info`), covering Checkpoints, UNet/Diffusion Models (Flux/SD3/Qwen/Wan), and GGUF loaders.
pub async fn fetch_comfy_models(
    client: &reqwest::Client,
    api_base: &str,
    api_key: Option<&str>,
) -> AppResult<Vec<String>> {
    let base = normalize_comfy_url(api_base)?;
    let url = format!("{}/object_info", base);

    let mut req = client.get(&url).timeout(Duration::from_secs(15));
    if let Some(k) = api_key.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        req = req.bearer_auth(k);
    }

    let resp = req.send().await.map_err(|e| {
        AppError::new(
            AppErrorCode::ProviderUnavailable,
            format!(
                "无法连接本地 ComfyUI 服务 ({})：{}。请确认 ComfyUI 已启动且端口正确。",
                base, e
            ),
        )
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::with_details(
            AppErrorCode::ProviderUnavailable,
            format!("ComfyUI /object_info 返回 HTTP {}", status.as_u16()),
            &text,
        ));
    }

    let info: Value = resp.json().await.map_err(|e| {
        AppError::new(
            AppErrorCode::ProviderBadResponse,
            format!("解析 ComfyUI /object_info 失败: {}", e),
        )
    })?;

    let mut models = Vec::new();

    // 1. Standard single-file checkpoints (SD1.5 / SDXL / Pony / Illustrious / Flux-FP8 checkpoint)
    for ckpt in extract_combo_options(&info, "CheckpointLoaderSimple", "ckpt_name") {
        models.push(ckpt);
    }

    // 2. Split UNet / Diffusion Models (Flux.1 Dev/Schnell, SD3.5, Qwen, Hunyuan, etc.)
    for unet in extract_combo_options(&info, "UNETLoader", "unet_name") {
        let tagged = format!("unet:{}", unet);
        if !models.contains(&tagged) {
            models.push(tagged);
        }
    }

    // 3. GGUF UNet Loader (if ComfyUI-GGUF is installed)
    for gguf in extract_combo_options(&info, "UnetLoaderGGUF", "unet_name") {
        let tagged = format!("gguf:{}", gguf);
        if !models.contains(&tagged) {
            models.push(tagged);
        }
    }

    // Always provide a virtual item so custom-workflow users can run without selecting a checkpoint
    if models.is_empty() {
        models.push("comfyui-custom-workflow".to_string());
    }

    Ok(models)
}

/// Extract embedded `prompt` or `workflow` JSON from ComfyUI PNG `tEXt` / `iTXt` chunks
pub fn extract_json_from_png_bytes(bytes: &[u8]) -> Option<(String, &'static str)> {
    const PNG_SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 8 || &bytes[0..8] != PNG_SIG {
        return None;
    }

    let mut pos = 8usize;
    let mut found_prompt: Option<String> = None;
    let mut found_workflow: Option<String> = None;

    while pos + 8 <= bytes.len() {
        let length = u32::from_be_bytes([
            bytes[pos],
            bytes[pos + 1],
            bytes[pos + 2],
            bytes[pos + 3],
        ]) as usize;
        let chunk_type = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start.saturating_add(length);
        if data_end > bytes.len() {
            break;
        }
        let chunk_data = &bytes[data_start..data_end];

        if chunk_type == b"tEXt" {
            if let Some(null_idx) = chunk_data.iter().position(|&b| b == 0) {
                let keyword = String::from_utf8_lossy(&chunk_data[..null_idx]);
                let text = String::from_utf8_lossy(&chunk_data[null_idx + 1..]).to_string();
                if keyword == "prompt" && text.trim().starts_with('{') {
                    found_prompt = Some(text);
                } else if keyword == "workflow" && text.trim().starts_with('{') {
                    found_workflow = Some(text);
                }
            }
        } else if chunk_type == b"iTXt" {
            if let Some(null_idx) = chunk_data.iter().position(|&b| b == 0) {
                let keyword = String::from_utf8_lossy(&chunk_data[..null_idx]);
                // Skip compression flag (1), compression method (1), language tag (null-term), translated keyword (null-term)
                let rest = &chunk_data[null_idx + 1..];
                if rest.len() >= 2 && rest[0] == 0 {
                    let after_flags = &rest[2..];
                    if let Some(lang_null) = after_flags.iter().position(|&b| b == 0) {
                        let after_lang = &after_flags[lang_null + 1..];
                        if let Some(trans_null) = after_lang.iter().position(|&b| b == 0) {
                            let text =
                                String::from_utf8_lossy(&after_lang[trans_null + 1..]).to_string();
                            if keyword == "prompt" && text.trim().starts_with('{') {
                                found_prompt = Some(text);
                            } else if keyword == "workflow" && text.trim().starts_with('{') {
                                found_workflow = Some(text);
                            }
                        }
                    }
                }
            }
        } else if chunk_type == b"IEND" {
            break;
        }

        pos = data_end.saturating_add(4); // skip 4-byte CRC
    }

    if let Some(p) = found_prompt {
        return Some((p, "png_embedded"));
    }
    if let Some(w) = found_workflow {
        return Some((w, "png_embedded"));
    }
    None
}

/// Check if a JSON Value is already in ComfyUI API Prompt format (`{"3": {"class_type": "KSampler", "inputs": {...}}}`)
pub fn is_api_prompt_format(val: &Value) -> bool {
    let Some(obj) = val.as_object() else {
        return false;
    };
    if obj.is_empty() || obj.contains_key("nodes") && obj.contains_key("links") {
        return false;
    }
    obj.values().any(|v| {
        v.as_object()
            .map(|n| n.contains_key("class_type") && n.contains_key("inputs"))
            .unwrap_or(false)
    })
}

/// Built-in fallback widget parameter names for common ComfyUI nodes when offline (ComfyUI server not yet started)
fn fallback_widget_names(class_type: &str) -> Option<Vec<&'static str>> {
    match class_type {
        "KSampler" => Some(vec![
            "seed",
            "control_after_generate",
            "steps",
            "cfg",
            "sampler_name",
            "scheduler",
            "denoise",
        ]),
        "KSamplerAdvanced" => Some(vec![
            "add_noise",
            "noise_seed",
            "control_after_generate",
            "steps",
            "cfg",
            "sampler_name",
            "scheduler",
            "start_at_step",
            "end_at_step",
            "return_with_leftover_noise",
        ]),
        "RandomNoise" => Some(vec!["noise_seed", "control_after_generate"]),
        "CheckpointLoaderSimple" | "CheckpointLoader" => Some(vec!["ckpt_name"]),
        "UNETLoader" | "UnetLoaderGGUF" => Some(vec!["unet_name", "weight_dtype"]),
        "CLIPLoader" | "CLIPLoaderGGUF" => Some(vec!["clip_name", "type", "device"]),
        "DualCLIPLoader" | "DualCLIPLoaderGGUF" => {
            Some(vec!["clip_name1", "clip_name2", "type", "device"])
        }
        "VAELoader" => Some(vec!["vae_name"]),
        "LoraLoader" => Some(vec![
            "lora_name",
            "strength_model",
            "strength_clip",
        ]),
        "LoraLoaderModelOnly" => Some(vec!["lora_name", "strength_model"]),
        "CLIPTextEncode" => Some(vec!["text"]),
        "CLIPTextEncodeSDXL" => Some(vec![
            "width",
            "height",
            "crop_w",
            "crop_h",
            "target_width",
            "target_height",
            "text_g",
            "text_l",
        ]),
        "FluxGuidance" => Some(vec!["guidance"]),
        "EmptyLatentImage" | "EmptySD3LatentImage" | "EmptyMochiLatentVideo" => {
            Some(vec!["width", "height", "batch_size"])
        }
        "LoadImage" => Some(vec!["image", "upload"]),
        "LoadImageMask" => Some(vec!["image", "channel", "upload"]),
        "SaveImage" => Some(vec!["filename_prefix"]),
        "PreviewImage" | "VAEDecode" | "VAEEncode" | "VAEEncodeForInpaint" => Some(vec![]),
        "ImageScale" => Some(vec![
            "upscale_method",
            "width",
            "height",
            "crop",
        ]),
        "ImageScaleBy" => Some(vec!["upscale_method", "scale_by"]),
        "BasicScheduler" => Some(vec!["scheduler", "steps", "denoise"]),
        "KSamplerSelect" => Some(vec!["sampler_name"]),
        "ModelSamplingFlux" => Some(vec!["max_shift", "base_shift", "width", "height"]),
        "ModelSamplingSD3" | "ModelSamplingAuraFlow" => Some(vec!["shift"]),
        _ => None,
    }
}

fn is_seed_control_token(val: &Value) -> bool {
    matches!(
        val.as_str(),
        Some("fixed" | "increment" | "decrement" | "randomize")
    )
}

/// Convert a standard ComfyUI Web UI workflow JSON (`nodes` + `links`) into the executable API Prompt dictionary.
/// Supports Reroute chains, PrimitiveNodes, Muted/Bypassed nodes, and `/object_info` widget mapping.
pub fn convert_ui_workflow_to_api(
    ui_json: &Value,
    object_info: Option<&Value>,
) -> AppResult<Value> {
    // If the user wrapped {"prompt": {...}}, unwrap it
    if let Some(inner) = ui_json.get("prompt") {
        if is_api_prompt_format(inner) {
            return Ok(inner.clone());
        }
    }
    if is_api_prompt_format(ui_json) {
        return Ok(ui_json.clone());
    }

    let nodes_arr = ui_json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            AppError::new(
                AppErrorCode::InvalidInput,
                "无效的 ComfyUI 工作流 JSON：既不是 API 格式，也不包含 nodes 数组",
            )
        })?;

    // Build link table: link_id -> (origin_node_id, origin_slot_index)
    let mut links_map: HashMap<i64, (String, i64)> = HashMap::new();
    if let Some(links_arr) = ui_json.get("links").and_then(|v| v.as_array()) {
        for link in links_arr {
            if let Some(arr) = link.as_array() {
                // ComfyUI link format: [link_id, origin_id, origin_slot, target_id, target_slot, type]
                if arr.len() >= 5 {
                    if let Some(link_id) = arr[0].as_i64() {
                        let origin_id = match &arr[1] {
                            Value::Number(n) => n.to_string(),
                            Value::String(s) => s.clone(),
                            _ => continue,
                        };
                        let origin_slot = arr[2].as_i64().unwrap_or(0);
                        links_map.insert(link_id, (origin_id, origin_slot));
                    }
                }
            }
        }
    }

    // Index all nodes by string ID
    let mut nodes_by_id: HashMap<String, &Value> = HashMap::new();
    for node in nodes_arr {
        let id_str = match node.get("id") {
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::String(s)) => s.clone(),
            _ => continue,
        };
        nodes_by_id.insert(id_str, node);
    }

    // Helper to recursively resolve a link through Reroute / Bypassed nodes or PrimitiveNode
    enum ResolvedLink {
        NodeOutput(String, i64),
        PrimitiveValue(Value),
        Ignored,
    }

    fn resolve_link(
        link_id: i64,
        links_map: &HashMap<i64, (String, i64)>,
        nodes_by_id: &HashMap<String, &Value>,
        depth: usize,
    ) -> ResolvedLink {
        if depth > 32 {
            return ResolvedLink::Ignored;
        }
        let Some((origin_id, origin_slot)) = links_map.get(&link_id) else {
            return ResolvedLink::Ignored;
        };
        let Some(origin_node) = nodes_by_id.get(origin_id) else {
            return ResolvedLink::Ignored;
        };
        let class_type = origin_node
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mode = origin_node.get("mode").and_then(|v| v.as_i64()).unwrap_or(0);

        // PrimitiveNode directly supplies its widget value
        if class_type == "PrimitiveNode" {
            if let Some(wv) = origin_node.get("widgets_values") {
                if let Some(arr) = wv.as_array() {
                    if let Some(first) = arr.first() {
                        return ResolvedLink::PrimitiveValue(first.clone());
                    }
                } else if let Some(obj) = wv.as_object() {
                    if let Some(first) = obj.get("value").or_else(|| obj.values().next()) {
                        return ResolvedLink::PrimitiveValue(first.clone());
                    }
                }
            }
            return ResolvedLink::Ignored;
        }

        // Reroute or Bypassed (mode == 4) or Muted (mode == 2)
        if class_type.starts_with("Reroute") || mode == 4 {
            if let Some(inputs) = origin_node.get("inputs").and_then(|v| v.as_array()) {
                // Find matching slot or first linked input
                let upstream_link = inputs
                    .get(*origin_slot as usize)
                    .and_then(|inp| inp.get("link").and_then(|l| l.as_i64()))
                    .or_else(|| {
                        inputs
                            .iter()
                            .find_map(|inp| inp.get("link").and_then(|l| l.as_i64()))
                    });
                if let Some(up_id) = upstream_link {
                    return resolve_link(up_id, links_map, nodes_by_id, depth + 1);
                }
            }
            return ResolvedLink::Ignored;
        }

        if mode == 2 {
            // Muted node
            return ResolvedLink::Ignored;
        }

        ResolvedLink::NodeOutput(origin_id.clone(), *origin_slot)
    }

    let mut api_prompt = Map::new();

    for node in nodes_arr {
        let id_str = match node.get("id") {
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::String(s)) => s.clone(),
            _ => continue,
        };
        let class_type = node
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if class_type.is_empty()
            || matches!(
                class_type,
                "Note" | "MarkdownNote" | "PrimitiveNode" | "Reroute" | "Reroute (rgthree)"
            )
        {
            continue;
        }

        // Skip Muted (2) or Bypassed (4) nodes
        let mode = node.get("mode").and_then(|v| v.as_i64()).unwrap_or(0);
        if mode == 2 || mode == 4 {
            continue;
        }

        let mut inputs_map = Map::new();
        let mut linked_input_names = HashSet::new();
        let mut node_widget_input_names: Vec<String> = Vec::new();

        // 1. Process explicit `inputs` array on the node (links + modern widget markers)
        if let Some(node_inputs) = node.get("inputs").and_then(|v| v.as_array()) {
            for inp in node_inputs {
                let Some(inp_name) = inp.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                if inp.get("widget").is_some() {
                    node_widget_input_names.push(inp_name.to_string());
                }
                if let Some(link_id) = inp.get("link").and_then(|v| v.as_i64()) {
                    linked_input_names.insert(inp_name.to_string());
                    match resolve_link(link_id, &links_map, &nodes_by_id, 0) {
                        ResolvedLink::NodeOutput(orig_id, orig_slot) => {
                            inputs_map.insert(
                                inp_name.to_string(),
                                json!([orig_id, orig_slot]),
                            );
                        }
                        ResolvedLink::PrimitiveValue(val) => {
                            inputs_map.insert(inp_name.to_string(), val);
                        }
                        ResolvedLink::Ignored => {}
                    }
                }
            }
        }

        // 2. Map `widgets_values` to widget inputs
        if let Some(wv) = node.get("widgets_values") {
            if let Some(wv_obj) = wv.as_object() {
                // Dictionary-style widgets_values (used by some newer ComfyUI nodes / VHS)
                for (k, v) in wv_obj {
                    if !linked_input_names.contains(k) {
                        inputs_map.insert(k.clone(), v.clone());
                    }
                }
            } else if let Some(wv_arr) = wv.as_array() {
                // Determine ordered widget parameter names from:
                // (a) `/object_info` schema if available,
                // (b) fallback_widget_names table, or
                // (c) `inputs[].widget` markers in the node JSON itself.
                let mut ordered_widget_names: Vec<(String, bool)> = Vec::new(); // (param_name, has_control_after_generate)

                if let Some(info_root) = object_info {
                    if let Some(node_schema) = info_root.get(class_type) {
                        let input_order = node_schema
                            .get("input_order")
                            .and_then(|io| io.as_object());
                        let input_obj = node_schema.get("input").and_then(|i| i.as_object());

                        let mut push_param = |p_name: &str, p_spec: &Value| {
                            let Some(spec_arr) = p_spec.as_array() else {
                                return;
                            };
                            let Some(first) = spec_arr.first() else {
                                return;
                            };
                            let is_widget = if first.is_array() {
                                true // COMBO dropdown
                            } else if let Some(type_str) = first.as_str() {
                                matches!(
                                    type_str,
                                    "INT" | "FLOAT" | "STRING" | "BOOLEAN" | "COMBO" | "IMAGEUPLOAD"
                                )
                            } else {
                                false
                            };
                            if is_widget {
                                let has_ctrl = spec_arr
                                    .get(1)
                                    .and_then(|cfg| cfg.get("control_after_generate"))
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false)
                                    || matches!(p_name, "seed" | "noise_seed");
                                ordered_widget_names.push((p_name.to_string(), has_ctrl));
                            }
                        };

                        if let Some(in_obj) = input_obj {
                            for section in ["required", "optional"] {
                                if let Some(sec_map) =
                                    in_obj.get(section).and_then(|s| s.as_object())
                                {
                                    if let Some(order_list) = input_order
                                        .and_then(|io| io.get(section))
                                        .and_then(|v| v.as_array())
                                    {
                                        for key_val in order_list {
                                            if let Some(k) = key_val.as_str() {
                                                if let Some(spec) = sec_map.get(k) {
                                                    push_param(k, spec);
                                                }
                                            }
                                        }
                                    } else {
                                        for (k, spec) in sec_map {
                                            push_param(k, spec);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if ordered_widget_names.is_empty() {
                    if let Some(fb) = fallback_widget_names(class_type) {
                        for name in fb {
                            if name == "control_after_generate" {
                                continue;
                            }
                            let has_ctrl = matches!(name, "seed" | "noise_seed");
                            ordered_widget_names.push((name.to_string(), has_ctrl));
                        }
                    } else if !node_widget_input_names.is_empty() {
                        for name in &node_widget_input_names {
                            let has_ctrl = matches!(name.as_str(), "seed" | "noise_seed");
                            ordered_widget_names.push((name.clone(), has_ctrl));
                        }
                    }
                }

                // Walk through `wv_arr` and assign values to `ordered_widget_names`
                let mut w_idx = 0usize;
                for (param_name, has_ctrl) in ordered_widget_names {
                    if w_idx >= wv_arr.len() {
                        break;
                    }
                    let val = &wv_arr[w_idx];
                    w_idx += 1;

                    // If this parameter has `control_after_generate` (like `seed` / `noise_seed`)
                    // and the next element in `widgets_values` is "fixed"/"randomize"/"increment"/"decrement", skip that UI-only token
                    if has_ctrl && w_idx < wv_arr.len() && is_seed_control_token(&wv_arr[w_idx]) {
                        w_idx += 1;
                    }

                    // Skip upload button virtual widget value if present
                    if param_name == "upload" {
                        continue;
                    }

                    if !linked_input_names.contains(&param_name) && !val.is_null() {
                        inputs_map.insert(param_name, val.clone());
                    }
                }
            }
        }

        let title = node
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(class_type);

        api_prompt.insert(
            id_str,
            json!({
                "class_type": class_type,
                "inputs": Value::Object(inputs_map),
                "_meta": {
                    "title": title
                }
            }),
        );
    }

    if api_prompt.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "转换后的 ComfyUI 工作流不包含任何有效执行节点",
        ));
    }

    Ok(Value::Object(api_prompt))
}

/// Analyze a raw workflow file (JSON string or PNG base64/bytes) and return discovered node slots + auto-suggested bindings.
pub async fn analyze_comfy_workflow(
    client: &reqwest::Client,
    api_base: &str,
    raw_content: &str,
) -> AppResult<ComfyWorkflowAnalysis> {
    let trimmed = raw_content.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "工作流内容为空",
        ));
    }

    // 1. Check if raw_content is a data:image/png;base64,... URL
    let (json_str, mut detected_format) = if let Some(idx) = trimmed.find(";base64,") {
        let b64 = &trimmed[idx + 8..];
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
            .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;
        extract_json_from_png_bytes(&bytes).ok_or_else(|| {
            AppError::new(
                AppErrorCode::InvalidInput,
                "该 PNG 图片中未检测到内嵌的 ComfyUI 工作流元数据（tEXt prompt/workflow）",
            )
        })?
    } else {
        (trimmed.to_string(), "api")
    };

    let parsed: Value = serde_json::from_str(&json_str).map_err(|e| {
        AppError::new(
            AppErrorCode::InvalidInput,
            format!("无法解析工作流 JSON: {}", e),
        )
    })?;

    // Try fetching `/object_info` from local ComfyUI if running (optional; falls back gracefully if offline)
    let object_info: Option<Value> = if let Ok(base) = normalize_comfy_url(api_base) {
        let info_url = format!("{}/object_info", base);
        if let Ok(resp) = client
            .get(&info_url)
            .timeout(Duration::from_secs(4))
            .send()
            .await
        {
            resp.json::<Value>().await.ok()
        } else {
            None
        }
    } else {
        None
    };

    let api_prompt = if is_api_prompt_format(&parsed) {
        parsed
    } else if parsed
        .get("prompt")
        .map(is_api_prompt_format)
        .unwrap_or(false)
    {
        parsed.get("prompt").unwrap().clone()
    } else {
        if detected_format == "api" {
            detected_format = "ui_converted";
        }
        convert_ui_workflow_to_api(&parsed, object_info.as_ref())?
    };

    Ok(inspect_api_prompt_slots(&api_prompt, detected_format))
}

/// Inspect an API-format prompt object to discover text, image, latent size, and seed slots, and automatically recommend bindings.
pub fn inspect_api_prompt_slots(api_prompt: &Value, format_tag: &str) -> ComfyWorkflowAnalysis {
    let mut text_nodes = Vec::new();
    let mut image_nodes = Vec::new();
    let mut latent_nodes = Vec::new();
    let mut seed_nodes = Vec::new();

    // Track which nodes are connected to KSampler's `positive` and `negative` inputs
    let mut sampler_positive_targets = HashSet::new();
    let mut sampler_negative_targets = HashSet::new();

    let Some(prompt_map) = api_prompt.as_object() else {
        return ComfyWorkflowAnalysis {
            format: format_tag.to_string(),
            node_count: 0,
            normalized_api_json: "{}".to_string(),
            text_nodes,
            image_nodes,
            latent_nodes,
            seed_nodes,
            suggested_positive_node_id: String::new(),
            suggested_positive_field: String::new(),
            suggested_negative_node_id: String::new(),
            suggested_negative_field: String::new(),
            suggested_image_node_id: String::new(),
            suggested_secondary_image_node_id: String::new(),
            suggested_latent_node_id: String::new(),
            suggested_seed_node_id: String::new(),
        };
    };

    // Helper to trace 1-2 hops from a sampler input link (e.g. KSampler.positive -> FluxGuidance -> CLIPTextEncode)
    let trace_upstream_id = |start_link: &Value| -> Option<String> {
        let mut curr_id = start_link.as_array()?.first()?.as_str()?.to_string();
        for _ in 0..4 {
            let Some(node) = prompt_map.get(&curr_id) else {
                break;
            };
            let ct = node.get("class_type").and_then(|v| v.as_str()).unwrap_or("");
            if ct.contains("TextEncode") || ct.contains("Prompt") || ct.contains("String") {
                return Some(curr_id);
            }
            if let Some(cond_link) = node
                .get("inputs")
                .and_then(|i| i.get("conditioning").or_else(|| i.get("positive")))
            {
                if let Some(next_id) = cond_link.as_array().and_then(|a| a.first()).and_then(|v| v.as_str()) {
                    curr_id = next_id.to_string();
                    continue;
                }
            }
            break;
        }
        Some(curr_id)
    };

    for (_nid, node) in prompt_map {
        if let Some(inputs) = node.get("inputs").and_then(|v| v.as_object()) {
            if let Some(pos_link) = inputs.get("positive").or_else(|| inputs.get("conditioning")) {
                if let Some(tid) = trace_upstream_id(pos_link) {
                    sampler_positive_targets.insert(tid);
                }
            }
            if let Some(neg_link) = inputs.get("negative") {
                if let Some(tid) = trace_upstream_id(neg_link) {
                    sampler_negative_targets.insert(tid);
                }
            }
        }
    }

    for (nid, node) in prompt_map {
        let class_type = node
            .get("class_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let title = node
            .get("_meta")
            .and_then(|m| m.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or(class_type)
            .to_string();
        let Some(inputs) = node.get("inputs").and_then(|v| v.as_object()) else {
            continue;
        };

        // 1. Check Text / Prompt fields
        for candidate_field in ["text", "text_g", "prompt", "positive", "string", "value"] {
            if let Some(Value::String(s)) = inputs.get(candidate_field) {
                // Skip filename_prefix or model names
                if class_type == "SaveImage"
                    || class_type.contains("Loader")
                    || s.ends_with(".safetensors")
                    || s.ends_with(".ckpt")
                    || s.ends_with(".pt")
                    || s.ends_with(".png")
                {
                    continue;
                }
                let preview: String = s.chars().take(60).collect();
                text_nodes.push(ComfyNodeSlotOption {
                    node_id: nid.clone(),
                    class_type: class_type.to_string(),
                    title: title.clone(),
                    field: candidate_field.to_string(),
                    preview,
                });
                break;
            }
        }

        // 2. Check Image Input nodes (`LoadImage`, `LoadImageMask`, etc.)
        if class_type == "LoadImage" || class_type == "LoadImageMask" || class_type.contains("LoadImage") {
            let preview = inputs
                .get("image")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            image_nodes.push(ComfyNodeSlotOption {
                node_id: nid.clone(),
                class_type: class_type.to_string(),
                title: title.clone(),
                field: "image".to_string(),
                preview,
            });
        }

        // 3. Check Latent / Canvas resolution nodes (`width` + `height`)
        if inputs.contains_key("width") && inputs.contains_key("height") {
            let w = inputs.get("width").and_then(|v| v.as_u64()).unwrap_or(1024);
            let h = inputs.get("height").and_then(|v| v.as_u64()).unwrap_or(1024);
            if class_type.contains("Latent") || class_type.contains("Empty") {
                latent_nodes.push(ComfyNodeSlotOption {
                    node_id: nid.clone(),
                    class_type: class_type.to_string(),
                    title: title.clone(),
                    field: "width,height".to_string(),
                    preview: format!("{}x{}", w, h),
                });
            }
        }

        // 4. Check Sampler / Noise Seed nodes (`seed` or `noise_seed`)
        for seed_field in ["seed", "noise_seed"] {
            if let Some(sv) = inputs.get(seed_field) {
                if sv.is_number() {
                    seed_nodes.push(ComfyNodeSlotOption {
                        node_id: nid.clone(),
                        class_type: class_type.to_string(),
                        title: title.clone(),
                        field: seed_field.to_string(),
                        preview: sv.to_string(),
                    });
                    break;
                }
            }
        }
    }

    // Recommend Positive & Negative prompt nodes
    let mut suggested_pos_id = String::new();
    let mut suggested_pos_field = String::new();
    let mut suggested_neg_id = String::new();
    let mut suggested_neg_field = String::new();

    for tn in &text_nodes {
        let title_lower = tn.title.to_lowercase();
        if sampler_positive_targets.contains(&tn.node_id) && suggested_pos_id.is_empty() {
            suggested_pos_id = tn.node_id.clone();
            suggested_pos_field = tn.field.clone();
        } else if sampler_negative_targets.contains(&tn.node_id) && suggested_neg_id.is_empty() {
            suggested_neg_id = tn.node_id.clone();
            suggested_neg_field = tn.field.clone();
        } else if (title_lower.contains("positive") || title_lower.contains("正向"))
            && suggested_pos_id.is_empty()
        {
            suggested_pos_id = tn.node_id.clone();
            suggested_pos_field = tn.field.clone();
        } else if (title_lower.contains("negative") || title_lower.contains("反向") || title_lower.contains("负面"))
            && suggested_neg_id.is_empty()
        {
            suggested_neg_id = tn.node_id.clone();
            suggested_neg_field = tn.field.clone();
        }
    }

    if suggested_pos_id.is_empty() {
        if let Some(first) = text_nodes.first() {
            suggested_pos_id = first.node_id.clone();
            suggested_pos_field = first.field.clone();
        }
    }
    if suggested_neg_id.is_empty() && text_nodes.len() >= 2 {
        if let Some(second) = text_nodes.iter().find(|n| n.node_id != suggested_pos_id) {
            suggested_neg_id = second.node_id.clone();
            suggested_neg_field = second.field.clone();
        }
    }

    let suggested_image_node_id = image_nodes
        .first()
        .map(|n| n.node_id.clone())
        .unwrap_or_default();
    let suggested_secondary_image_node_id = image_nodes
        .get(1)
        .map(|n| n.node_id.clone())
        .unwrap_or_default();
    let suggested_latent_node_id = latent_nodes
        .first()
        .map(|n| n.node_id.clone())
        .unwrap_or_default();
    let suggested_seed_node_id = seed_nodes
        .first()
        .map(|n| n.node_id.clone())
        .unwrap_or_default();

    ComfyWorkflowAnalysis {
        format: format_tag.to_string(),
        node_count: prompt_map.len(),
        normalized_api_json: serde_json::to_string_pretty(api_prompt).unwrap_or_default(),
        text_nodes,
        image_nodes,
        latent_nodes,
        seed_nodes,
        suggested_positive_node_id: suggested_pos_id,
        suggested_positive_field: suggested_pos_field,
        suggested_negative_node_id: suggested_neg_id,
        suggested_negative_field: suggested_neg_field,
        suggested_image_node_id,
        suggested_secondary_image_node_id,
        suggested_latent_node_id,
        suggested_seed_node_id,
    }
}

/// Upload an input image (sketch / character anchor / mask) to ComfyUI's `POST /upload/image` endpoint.
pub async fn upload_image_to_comfy(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    bytes: Vec<u8>,
    prefix: &str,
) -> AppResult<String> {
    let short_id = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let filename = format!("s2a_{}_{}.png", prefix, short_id);
    let url = format!("{}/upload/image", base_url);

    let file_part = multipart::Part::bytes(bytes)
        .file_name(filename.clone())
        .mime_str("image/png")
        .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;

    let form = multipart::Form::new()
        .part("image", file_part)
        .text("type", "input")
        .text("overwrite", "true");

    let mut req = client.post(&url).multipart(form);
    if let Some(k) = api_key.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        req = req.bearer_auth(k);
    }

    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::with_details(
            AppErrorCode::ProviderUnavailable,
            format!("ComfyUI /upload/image 上传图片失败 (HTTP {})", status.as_u16()),
            &text,
        ));
    }

    let val: Value = serde_json::from_str(&text).unwrap_or_default();
    let server_name = val
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&filename)
        .to_string();
    let subfolder = val
        .get("subfolder")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    if subfolder.is_empty() {
        Ok(server_name)
    } else {
        Ok(format!("{}/{}", subfolder, server_name))
    }
}

fn generate_random_seed() -> u64 {
    let b = uuid::Uuid::new_v4();
    let bytes = b.as_bytes();
    let raw = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    raw % 9_007_199_254_740_991 // Safe JS integer range (< 2^53)
}

/// Build an adaptive built-in ComfyUI API workflow for users who directly selected a local model without importing a `.json`.
/// Supports:
/// - Standard Checkpoints (SD1.5 / SDXL / Pony / Illustrious)
/// - Flux.1 All-in-One Checkpoints (`flux1-dev-fp8.safetensors` / `flux1-schnell`)
/// - Split UNet (`unet:...`) & GGUF (`gguf:...`) models by auto-matching local CLIP & VAE via `/object_info`
/// - Txt2Img, Sketch/Reference Img2Img (`VAEEncode`), and Mask Inpaint (`SetLatentNoiseMask`)
async fn build_adaptive_builtin_prompt(
    client: &reqwest::Client,
    base_url: &str,
    model_spec: &str,
    positive_prompt: &str,
    negative_prompt: &str,
    width: u32,
    height: u32,
    uploaded_image_name: Option<&str>,
    uploaded_mask_name: Option<&str>,
    denoise_strength: f64,
) -> AppResult<Value> {
    let model_clean = model_spec.trim();
    let lower = model_clean.to_lowercase();
    let is_flux = lower.contains("flux");
    let is_schnell = lower.contains("schnell") || lower.contains("turbo") || lower.contains("lightning");
    let is_sd3 = lower.contains("sd3");

    let steps = if is_schnell { 4 } else { 22 };
    let cfg = if is_flux {
        1.0
    } else if is_sd3 {
        4.5
    } else if is_schnell {
        2.0
    } else {
        7.0
    };
    let sampler_name = if is_flux || is_sd3 || is_schnell {
        "euler"
    } else {
        "dpmpp_2m"
    };
    let scheduler = if is_flux {
        "simple"
    } else if is_sd3 {
        "sgm_uniform"
    } else if is_schnell {
        "normal"
    } else {
        "karras"
    };

    let seed = generate_random_seed();
    let mut prompt_map = Map::new();

    // Determine how Model ("1"), CLIP ("1" or "2"), and VAE ("1" or "3") are loaded
    let (model_ref, clip_ref, vae_ref) = if model_clean.starts_with("unet:")
        || model_clean.starts_with("gguf:")
    {
        let is_gguf = model_clean.starts_with("gguf:");
        let unet_name = model_clean
            .split_once(':')
            .map(|x| x.1)
            .unwrap_or(model_clean);

        // Query `/object_info` to find available CLIP and VAE files on the user's machine
        let info_url = format!("{}/object_info", base_url);
        let info: Value = if let Ok(r) = client
            .get(&info_url)
            .timeout(Duration::from_secs(6))
            .send()
            .await
        {
            r.json::<Value>().await.unwrap_or_default()
        } else {
            Value::Null
        };

        if is_gguf {
            prompt_map.insert(
                "1".to_string(),
                json!({
                    "class_type": "UnetLoaderGGUF",
                    "inputs": { "unet_name": unet_name }
                }),
            );
        } else {
            prompt_map.insert(
                "1".to_string(),
                json!({
                    "class_type": "UNETLoader",
                    "inputs": {
                        "unet_name": unet_name,
                        "weight_dtype": "default"
                    }
                }),
            );
        }

        // Find DualCLIP or single CLIP
        let dual_clips = extract_combo_options(&info, "DualCLIPLoader", "clip_name1");
        let single_clips = extract_combo_options(&info, "CLIPLoader", "clip_name");
        let vaes = extract_combo_options(&info, "VAELoader", "vae_name");

        if is_flux && dual_clips.len() >= 2 {
            let t5 = dual_clips
                .iter()
                .find(|c| c.to_lowercase().contains("t5"))
                .cloned()
                .unwrap_or_else(|| dual_clips[0].clone());
            let clip_l = dual_clips
                .iter()
                .find(|c| c.to_lowercase().contains("clip_l"))
                .cloned()
                .unwrap_or_else(|| dual_clips[1].clone());
            prompt_map.insert(
                "2".to_string(),
                json!({
                    "class_type": "DualCLIPLoader",
                    "inputs": {
                        "clip_name1": t5,
                        "clip_name2": clip_l,
                        "type": "flux"
                    }
                }),
            );
        } else if let Some(first_clip) = single_clips.first() {
            let clip_type = if is_flux {
                "flux"
            } else if is_sd3 {
                "sd3"
            } else {
                "stable_diffusion"
            };
            prompt_map.insert(
                "2".to_string(),
                json!({
                    "class_type": "CLIPLoader",
                    "inputs": {
                        "clip_name": first_clip,
                        "type": clip_type
                    }
                }),
            );
        } else {
            return Err(AppError::new(
                AppErrorCode::ModelMissing,
                "选择了拆分版 UNet 模型，但未在 ComfyUI 的 models/clip 目录检测到可用的 CLIP 文本编码器，建议直接导入完整工作流 (.json)。",
            ));
        }

        let selected_vae = if is_flux {
            vaes.iter()
                .find(|v| {
                    let l = v.to_lowercase();
                    l.contains("ae") || l.contains("flux")
                })
                .or_else(|| vaes.first())
                .cloned()
        } else {
            vaes.first().cloned()
        };

        let Some(vae_file) = selected_vae else {
            return Err(AppError::new(
                AppErrorCode::ModelMissing,
                "选择了拆分版 UNet 模型，但未在 ComfyUI 的 models/vae 目录检测到可用的 VAE 模型，建议直接导入完整工作流 (.json)。",
            ));
        };

        prompt_map.insert(
            "3".to_string(),
            json!({
                "class_type": "VAELoader",
                "inputs": { "vae_name": vae_file }
            }),
        );

        (json!(["1", 0]), json!(["2", 0]), json!(["3", 0]))
    } else {
        // Standard CheckpointLoaderSimple (single file containing Model + CLIP + VAE)
        let ckpt_name = if model_clean.is_empty() {
            // Auto-pick first checkpoint if user left model empty
            let list = fetch_comfy_models(client, base_url, None).await?;
            list.into_iter()
                .find(|m| !m.starts_with("unet:") && !m.starts_with("gguf:"))
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::ModelMissing,
                        "本地 ComfyUI models/checkpoints 目录为空，请先放置模型或切换到【自定义工作流 (.json)】模式。",
                    )
                })?
        } else {
            model_clean.to_string()
        };

        prompt_map.insert(
            "1".to_string(),
            json!({
                "class_type": "CheckpointLoaderSimple",
                "inputs": { "ckpt_name": ckpt_name }
            }),
        );

        (json!(["1", 0]), json!(["1", 1]), json!(["1", 2]))
    };

    // Node 6: Positive Prompt
    prompt_map.insert(
        "6".to_string(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": positive_prompt,
                "clip": clip_ref.clone()
            }
        }),
    );

    // Node 7: Negative Prompt
    prompt_map.insert(
        "7".to_string(),
        json!({
            "class_type": "CLIPTextEncode",
            "inputs": {
                "text": if is_flux { "" } else { negative_prompt },
                "clip": clip_ref
            }
        }),
    );

    // Latent source: either EmptyLatentImage ("5") or LoadImage ("10") -> ImageScale ("11") -> VAEEncode ("12")
    let (latent_ref, actual_denoise) = if let Some(img_name) = uploaded_image_name {
        prompt_map.insert(
            "10".to_string(),
            json!({
                "class_type": "LoadImage",
                "inputs": { "image": img_name }
            }),
        );
        prompt_map.insert(
            "11".to_string(),
            json!({
                "class_type": "ImageScale",
                "inputs": {
                    "image": ["10", 0],
                    "upscale_method": "bicubic",
                    "width": width,
                    "height": height,
                    "crop": "center"
                }
            }),
        );
        prompt_map.insert(
            "12".to_string(),
            json!({
                "class_type": "VAEEncode",
                "inputs": {
                    "pixels": ["11", 0],
                    "vae": vae_ref.clone()
                }
            }),
        );

        if let Some(mask_name) = uploaded_mask_name {
            prompt_map.insert(
                "13".to_string(),
                json!({
                    "class_type": "LoadImageMask",
                    "inputs": {
                        "image": mask_name,
                        "channel": "red"
                    }
                }),
            );
            prompt_map.insert(
                "14".to_string(),
                json!({
                    "class_type": "SetLatentNoiseMask",
                    "inputs": {
                        "samples": ["12", 0],
                        "mask": ["13", 0]
                    }
                }),
            );
            (json!(["14", 0]), 0.88)
        } else {
            (json!(["12", 0]), denoise_strength.clamp(0.15, 0.98))
        }
    } else {
        prompt_map.insert(
            "5".to_string(),
            json!({
                "class_type": "EmptyLatentImage",
                "inputs": {
                    "width": width,
                    "height": height,
                    "batch_size": 1
                }
            }),
        );
        (json!(["5", 0]), 1.0)
    };

    // Node 8: KSampler
    prompt_map.insert(
        "8".to_string(),
        json!({
            "class_type": "KSampler",
            "inputs": {
                "seed": seed,
                "steps": steps,
                "cfg": cfg,
                "sampler_name": sampler_name,
                "scheduler": scheduler,
                "denoise": actual_denoise,
                "model": model_ref,
                "positive": ["6", 0],
                "negative": ["7", 0],
                "latent_image": latent_ref
            }
        }),
    );

    // Node 9: VAEDecode
    prompt_map.insert(
        "9".to_string(),
        json!({
            "class_type": "VAEDecode",
            "inputs": {
                "samples": ["8", 0],
                "vae": vae_ref
            }
        }),
    );

    // Node 15: SaveImage
    prompt_map.insert(
        "15".to_string(),
        json!({
            "class_type": "SaveImage",
            "inputs": {
                "filename_prefix": "Sketch2Asset",
                "images": ["9", 0]
            }
        }),
    );

    Ok(Value::Object(prompt_map))
}

/// Inject Sketch2Asset parameters (prompt, negative prompt, uploaded reference images, size, and random seed)
/// into a user-imported ComfyUI workflow.
fn inject_params_into_custom_workflow(
    mut api_prompt: Value,
    cfg: &ComfyWorkflowConfig,
    positive_prompt: &str,
    negative_prompt: &str,
    width: u32,
    height: u32,
    uploaded_primary_img: Option<&str>,
    uploaded_secondary_img: Option<&str>,
) -> AppResult<Value> {
    let analysis = inspect_api_prompt_slots(&api_prompt, "api");
    let Some(prompt_map) = api_prompt.as_object_mut() else {
        return Err(AppError::new(
            AppErrorCode::InvalidInput,
            "ComfyUI API 工作流结构不是有效的 JSON 对象",
        ));
    };

    // 0. Replace `{{prompt}}` or `{{negative_prompt}}` placeholders anywhere in string inputs
    for (_nid, node) in prompt_map.iter_mut() {
        if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
            for (_k, v) in inputs.iter_mut() {
                if let Some(s) = v.as_str() {
                    if s.contains("{{prompt}}") || s.contains("{{negative_prompt}}") {
                        let replaced = s
                            .replace("{{prompt}}", positive_prompt)
                            .replace("{{negative_prompt}}", negative_prompt);
                        *v = Value::String(replaced);
                    }
                }
            }
        }
    }

    // 1. Positive Prompt injection
    let pos_node_id = if !cfg.positive_node_id.trim().is_empty() {
        cfg.positive_node_id.trim().to_string()
    } else {
        analysis.suggested_positive_node_id.clone()
    };
    let pos_field = if !cfg.positive_field.trim().is_empty() {
        cfg.positive_field.trim().to_string()
    } else if !analysis.suggested_positive_field.is_empty() {
        analysis.suggested_positive_field.clone()
    } else {
        "text".to_string()
    };

    if !pos_node_id.is_empty() {
        if let Some(node) = prompt_map.get_mut(&pos_node_id) {
            if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
                inputs.insert(pos_field, Value::String(positive_prompt.to_string()));
                // If it's CLIPTextEncodeSDXL with both text_g and text_l
                if inputs.contains_key("text_l") {
                    inputs.insert("text_l".to_string(), Value::String(positive_prompt.to_string()));
                }
            }
        }
    }

    // 2. Negative Prompt injection
    if !negative_prompt.trim().is_empty() {
        let neg_node_id = if !cfg.negative_node_id.trim().is_empty() {
            cfg.negative_node_id.trim().to_string()
        } else {
            analysis.suggested_negative_node_id.clone()
        };
        let neg_field = if !cfg.negative_field.trim().is_empty() {
            cfg.negative_field.trim().to_string()
        } else if !analysis.suggested_negative_field.is_empty() {
            analysis.suggested_negative_field.clone()
        } else {
            "text".to_string()
        };

        if !neg_node_id.is_empty() && neg_node_id != pos_node_id {
            if let Some(node) = prompt_map.get_mut(&neg_node_id) {
                if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
                    inputs.insert(neg_field, Value::String(negative_prompt.to_string()));
                }
            }
        }
    }

    // 3. Canvas Resolution (Latent Width & Height) injection
    let latent_id = if !cfg.latent_node_id.trim().is_empty() {
        cfg.latent_node_id.trim().to_string()
    } else {
        analysis.suggested_latent_node_id.clone()
    };
    if !latent_id.is_empty() {
        if let Some(node) = prompt_map.get_mut(&latent_id) {
            if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
                if inputs.contains_key("width") && inputs.contains_key("height") {
                    inputs.insert("width".to_string(), json!(width));
                    inputs.insert("height".to_string(), json!(height));
                }
            }
        }
    }

    // 4. Primary Reference / Sketch Image injection
    if let Some(img_name) = uploaded_primary_img {
        let img_node_id = if !cfg.image_node_id.trim().is_empty() {
            cfg.image_node_id.trim().to_string()
        } else {
            analysis.suggested_image_node_id.clone()
        };

        if !img_node_id.is_empty() && prompt_map.contains_key(&img_node_id) {
            if let Some(node) = prompt_map.get_mut(&img_node_id) {
                if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
                    inputs.insert("image".to_string(), Value::String(img_name.to_string()));
                }
            }
        } else if !latent_id.is_empty() {
            // The user imported a pure Txt2Img workflow (no LoadImage node),
            // but drew a sketch on the canvas! Dynamically bridge LoadImage -> ImageScale -> VAEEncode!
            let vae_link = prompt_map.values().find_map(|n| {
                n.get("inputs")
                    .and_then(|i| i.get("vae"))
                    .filter(|v| v.is_array())
                    .cloned()
            });

            if let Some(vae_src) = vae_link {
                prompt_map.insert(
                    "_s2a_load_img".to_string(),
                    json!({
                        "class_type": "LoadImage",
                        "inputs": { "image": img_name }
                    }),
                );
                prompt_map.insert(
                    "_s2a_scale_img".to_string(),
                    json!({
                        "class_type": "ImageScale",
                        "inputs": {
                            "image": ["_s2a_load_img", 0],
                            "upscale_method": "bicubic",
                            "width": width,
                            "height": height,
                            "crop": "center"
                        }
                    }),
                );
                prompt_map.insert(
                    "_s2a_vae_encode".to_string(),
                    json!({
                        "class_type": "VAEEncode",
                        "inputs": {
                            "pixels": ["_s2a_scale_img", 0],
                            "vae": vae_src
                        }
                    }),
                );

                // Rewire any sampler node that referenced `[latent_id, 0]` to `["_s2a_vae_encode", 0]` and set denoise
                let denoise_val = cfg.denoise_strength.clamp(0.15, 0.98);
                for (_nid, node) in prompt_map.iter_mut() {
                    if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
                        let uses_latent = inputs
                            .get("latent_image")
                            .or_else(|| inputs.get("samples"))
                            .and_then(|v| v.as_array())
                            .and_then(|a| a.first())
                            .and_then(|v| v.as_str())
                            .map(|s| s == latent_id)
                            .unwrap_or(false);
                        if uses_latent {
                            if inputs.contains_key("latent_image") {
                                inputs.insert(
                                    "latent_image".to_string(),
                                    json!(["_s2a_vae_encode", 0]),
                                );
                            }
                            if inputs.contains_key("denoise") {
                                inputs.insert("denoise".to_string(), json!(denoise_val));
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Secondary Reference / Mask Image injection
    if let Some(sec_name) = uploaded_secondary_img {
        let sec_node_id = if !cfg.secondary_image_node_id.trim().is_empty() {
            cfg.secondary_image_node_id.trim().to_string()
        } else {
            analysis.suggested_secondary_image_node_id.clone()
        };
        if !sec_node_id.is_empty() {
            if let Some(node) = prompt_map.get_mut(&sec_node_id) {
                if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
                    inputs.insert("image".to_string(), Value::String(sec_name.to_string()));
                }
            }
        }
    }

    // 6. Randomize Seed on Sampler / Noise nodes so ComfyUI doesn't return cached identical output
    let new_seed = generate_random_seed();
    let target_seed_id = cfg.seed_node_id.trim();
    for (nid, node) in prompt_map.iter_mut() {
        if !target_seed_id.is_empty() && nid != target_seed_id {
            continue;
        }
        if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_object_mut()) {
            if inputs.get("seed").map(|v| v.is_number()).unwrap_or(false) {
                inputs.insert("seed".to_string(), json!(new_seed));
            }
            if inputs
                .get("noise_seed")
                .map(|v| v.is_number())
                .unwrap_or(false)
            {
                inputs.insert("noise_seed".to_string(), json!(new_seed));
            }
        }
    }

    // 7. Ensure there is at least one SaveImage or PreviewImage output node
    let has_output_node = prompt_map.values().any(|n| {
        let ct = n.get("class_type").and_then(|v| v.as_str()).unwrap_or("");
        ct.contains("SaveImage") || ct.contains("PreviewImage") || ct.contains("ImageOutput")
    });
    if !has_output_node {
        // Find a VAEDecode node and attach SaveImage
        let decode_id = prompt_map.iter().find_map(|(k, n)| {
            let ct = n.get("class_type").and_then(|v| v.as_str()).unwrap_or("");
            if ct == "VAEDecode" {
                Some(k.clone())
            } else {
                None
            }
        });
        if let Some(did) = decode_id {
            prompt_map.insert(
                "_s2a_save".to_string(),
                json!({
                    "class_type": "SaveImage",
                    "inputs": {
                        "filename_prefix": "Sketch2Asset",
                        "images": [did, 0]
                    }
                }),
            );
        }
    }

    Ok(api_prompt)
}

/// Submit an API prompt to ComfyUI `POST /prompt`, poll `/history/{prompt_id}`, and fetch the generated image bytes via `/view`.
pub async fn execute_comfy_prompt(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    api_prompt: Value,
    via_label: String,
) -> AppResult<(Vec<u8>, String)> {
    let client_id = uuid::Uuid::new_v4().to_string();
    let prompt_url = format!("{}/prompt", base_url);

    let payload = json!({
        "prompt": api_prompt,
        "client_id": client_id
    });

    let mut req = client.post(&prompt_url).json(&payload);
    if let Some(k) = api_key.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        req = req.bearer_auth(k);
    }

    let resp = req.send().await.map_err(|e| {
        AppError::new(
            AppErrorCode::ProviderUnavailable,
            format!("无法向本地 ComfyUI ({}) 提交工作流任务: {}", base_url, e),
        )
    })?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        // Parse node_errors if present for friendly troubleshooting
        let mut detail_msg = format!("ComfyUI 拒绝了工作流请求 (HTTP {})", status.as_u16());
        if let Ok(err_json) = serde_json::from_str::<Value>(&text) {
            if let Some(err_obj) = err_json.get("error") {
                if let Some(m) = err_obj.get("message").and_then(|v| v.as_str()) {
                    detail_msg = format!("ComfyUI 错误: {}", m);
                }
            }
            if let Some(ne) = err_json.get("node_errors").and_then(|v| v.as_object()) {
                if let Some((nid, nerr)) = ne.iter().next() {
                    let ctype = nerr
                        .get("class_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Node");
                    let first_msg = nerr
                        .get("errors")
                        .and_then(|e| e.as_array())
                        .and_then(|a| a.first())
                        .and_then(|x| x.get("details").or_else(|| x.get("message")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    detail_msg = format!("ComfyUI 节点 #{} ({}) 报错: {}", nid, ctype, first_msg);
                }
            }
        }
        return Err(AppError::with_details(
            AppErrorCode::ProviderBadResponse,
            detail_msg,
            &text,
        ));
    }

    let resp_json: Value = serde_json::from_str(&text).map_err(|e| {
        AppError::with_details(
            AppErrorCode::ProviderBadResponse,
            format!("ComfyUI /prompt 返回的不是合法 JSON: {}", e),
            &text,
        )
    })?;

    let prompt_id = resp_json
        .get("prompt_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::with_details(
                AppErrorCode::ProviderBadResponse,
                "ComfyUI /prompt 未返回 prompt_id",
                &text,
            )
        })?
        .to_string();

    // Poll `/history/{prompt_id}` until execution finishes (timeout: 600s for heavy local models)
    let history_url = format!("{}/history/{}", base_url, prompt_id);
    let start_wait = Instant::now();
    let max_wait = Duration::from_secs(600);

    loop {
        if start_wait.elapsed() > max_wait {
            return Err(AppError::new(
                AppErrorCode::ProviderUnavailable,
                "ComfyUI 本地生图超时（超过 10 分钟），请检查 ComfyUI 控制台是否有显存不足或卡死。",
            ));
        }

        tokio::time::sleep(Duration::from_millis(650)).await;

        let mut h_req = client.get(&history_url).timeout(Duration::from_secs(10));
        if let Some(k) = api_key.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            h_req = h_req.bearer_auth(k);
        }

        let Ok(h_resp) = h_req.send().await else {
            continue;
        };
        if !h_resp.status().is_success() {
            continue;
        }
        let Ok(h_json) = h_resp.json::<Value>().await else {
            continue;
        };

        let Some(entry) = h_json.get(&prompt_id) else {
            // Still in queue or executing
            continue;
        };

        // Check if ComfyUI reported an execution error in status.messages
        if let Some(status_obj) = entry.get("status") {
            let status_str = status_obj
                .get("status_str")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if status_str == "error" {
                let mut err_detail = "ComfyUI 工作流执行出错".to_string();
                if let Some(msgs) = status_obj.get("messages").and_then(|v| v.as_array()) {
                    for m in msgs {
                        if let Some(arr) = m.as_array() {
                            if arr.first().and_then(|v| v.as_str()) == Some("execution_error") {
                                if let Some(data) = arr.get(1) {
                                    let node_type = data
                                        .get("node_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Node");
                                    let exc_msg = data
                                        .get("exception_message")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("未知异常");
                                    err_detail =
                                        format!("ComfyUI [{}] 执行异常: {}", node_type, exc_msg.trim());
                                }
                            }
                        }
                    }
                }
                return Err(AppError::new(AppErrorCode::ProviderBadResponse, err_detail));
            }
        }

        // Look for generated images in `outputs`
        if let Some(outputs) = entry.get("outputs").and_then(|v| v.as_object()) {
            // Prefer `"output"` type (SaveImage) over `"temp"` (PreviewImage)
            let mut candidate_imgs: Vec<(String, String, String)> = Vec::new();
            for (_node_id, node_out) in outputs {
                if let Some(imgs) = node_out.get("images").and_then(|v| v.as_array()) {
                    for im in imgs {
                        let fname = im
                            .get("filename")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let subfolder = im
                            .get("subfolder")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let ftype = im
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("output")
                            .to_string();
                        if !fname.is_empty() {
                            candidate_imgs.push((fname, subfolder, ftype));
                        }
                    }
                }
            }

            // Sort so `type == "output"` comes first
            candidate_imgs.sort_by_key(|x| if x.2 == "output" { 0 } else { 1 });

            if let Some((filename, subfolder, ftype)) = candidate_imgs.first() {
                let view_url = format!("{}/view", base_url);
                let mut v_req = client.get(&view_url).query(&[
                    ("filename", filename.as_str()),
                    ("subfolder", subfolder.as_str()),
                    ("type", ftype.as_str()),
                ]);
                if let Some(k) = api_key.map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    v_req = v_req.bearer_auth(k);
                }

                let img_resp = v_req.send().await?;
                if !img_resp.status().is_success() {
                    return Err(AppError::new(
                        AppErrorCode::ProviderBadResponse,
                        format!("从 ComfyUI /view 下载结果图失败: HTTP {}", img_resp.status()),
                    ));
                }
                let bytes = img_resp.bytes().await?.to_vec();
                return Ok((bytes, via_label));
            }
        }
    }
}

/// Main entry point for ComfyUI generation (supports both Built-in Local Model Pipeline and Custom Imported Workflow).
pub async fn generate_via_comfyui(
    client: &reqwest::Client,
    api_base: &str,
    api_key: Option<&str>,
    model: &str,
    positive_prompt: &str,
    negative_prompt: &str,
    size: &str,
    primary_image: Option<(Vec<u8>, String)>,
    secondary_image: Option<(Vec<u8>, String)>,
    comfy_cfg: Option<&ComfyWorkflowConfig>,
) -> AppResult<(Vec<u8>, String)> {
    let base_url = normalize_comfy_url(api_base)?;
    let (width, height) = parse_size_dims(size).unwrap_or((1024, 1024));
    let default_cfg = ComfyWorkflowConfig::default();
    let cfg = comfy_cfg.unwrap_or(&default_cfg);

    // Upload primary / secondary images if provided
    let uploaded_primary = if let Some((bytes, _mime)) = primary_image {
        Some(upload_image_to_comfy(client, &base_url, api_key, bytes, "ref").await?)
    } else {
        None
    };

    let uploaded_secondary = if let Some((bytes, _mime)) = secondary_image {
        Some(upload_image_to_comfy(client, &base_url, api_key, bytes, "sec").await?)
    } else {
        None
    };

    let use_custom = cfg.mode == "custom" && !cfg.raw_workflow_json.trim().is_empty();

    let (api_prompt, via_label) = if use_custom {
        let parsed: Value = serde_json::from_str(cfg.raw_workflow_json.trim()).map_err(|e| {
            AppError::new(
                AppErrorCode::InvalidInput,
                format!("保存的自定义 ComfyUI 工作流 JSON 无法解析: {}", e),
            )
        })?;

        let base_api_prompt = if is_api_prompt_format(&parsed) {
            parsed
        } else {
            // Query live `/object_info` from running ComfyUI server to ensure 100% accurate conversion of custom nodes
            let info_url = format!("{}/object_info", base_url);
            let obj_info = if let Ok(r) = client
                .get(&info_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                r.json::<Value>().await.ok()
            } else {
                None
            };
            convert_ui_workflow_to_api(&parsed, obj_info.as_ref())?
        };

        let injected = inject_params_into_custom_workflow(
            base_api_prompt,
            cfg,
            positive_prompt,
            negative_prompt,
            width,
            height,
            uploaded_primary.as_deref(),
            uploaded_secondary.as_deref(),
        )?;

        let wf_label = if cfg.workflow_name.trim().is_empty() {
            "comfyui (workflow)".to_string()
        } else {
            format!("comfyui ({})", cfg.workflow_name.trim())
        };
        (injected, wf_label)
    } else {
        let built = build_adaptive_builtin_prompt(
            client,
            &base_url,
            model,
            positive_prompt,
            negative_prompt,
            width,
            height,
            uploaded_primary.as_deref(),
            uploaded_secondary.as_deref(),
            cfg.denoise_strength,
        )
        .await?;
        (built, "comfyui (builtin)".to_string())
    };

    execute_comfy_prompt(client, &base_url, api_key, api_prompt, via_label).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_workflow_to_api_conversion_and_slot_detection() {
        let ui_workflow = serde_json::json!({
            "last_node_id": 9,
            "last_link_id": 9,
            "nodes": [
                {
                    "id": 4,
                    "type": "CheckpointLoaderSimple",
                    "widgets_values": ["sd_xl_base_1.0.safetensors"]
                },
                {
                    "id": 5,
                    "type": "EmptyLatentImage",
                    "widgets_values": [1024, 1024, 1]
                },
                {
                    "id": 6,
                    "type": "CLIPTextEncode",
                    "title": "CLIP Text Encode (Positive)",
                    "inputs": [{"name": "clip", "type": "CLIP", "link": 3}],
                    "widgets_values": ["a magical crystal sword"]
                },
                {
                    "id": 7,
                    "type": "CLIPTextEncode",
                    "title": "CLIP Text Encode (Negative)",
                    "inputs": [{"name": "clip", "type": "CLIP", "link": 4}],
                    "widgets_values": ["blurry, watermark"]
                },
                {
                    "id": 3,
                    "type": "KSampler",
                    "inputs": [
                        {"name": "model", "type": "MODEL", "link": 1},
                        {"name": "positive", "type": "CONDITIONING", "link": 5},
                        {"name": "negative", "type": "CONDITIONING", "link": 6},
                        {"name": "latent_image", "type": "LATENT", "link": 2}
                    ],
                    "widgets_values": [123456789, "randomize", 20, 7.5, "euler", "normal", 1.0]
                },
                {
                    "id": 8,
                    "type": "VAEDecode",
                    "inputs": [
                        {"name": "samples", "type": "LATENT", "link": 7},
                        {"name": "vae", "type": "VAE", "link": 8}
                    ]
                },
                {
                    "id": 9,
                    "type": "SaveImage",
                    "inputs": [
                        {"name": "images", "type": "IMAGE", "link": 9}
                    ],
                    "widgets_values": ["ComfyUI"]
                }
            ],
            "links": [
                [1, 4, 0, 3, 0, "MODEL"],
                [2, 5, 0, 3, 3, "LATENT"],
                [3, 4, 1, 6, 0, "CLIP"],
                [4, 4, 1, 7, 0, "CLIP"],
                [5, 6, 0, 3, 1, "CONDITIONING"],
                [6, 7, 0, 3, 2, "CONDITIONING"],
                [7, 3, 0, 8, 0, "LATENT"],
                [8, 4, 2, 8, 1, "VAE"],
                [9, 8, 0, 9, 0, "IMAGE"]
            ]
        });

        let api_prompt = convert_ui_workflow_to_api(&ui_workflow, None).expect("convert ok");
        assert!(is_api_prompt_format(&api_prompt));

        let analysis = inspect_api_prompt_slots(&api_prompt, "ui_converted");
        assert_eq!(analysis.suggested_positive_node_id, "6");
        assert_eq!(analysis.suggested_negative_node_id, "7");
        assert_eq!(analysis.suggested_latent_node_id, "5");
        assert_eq!(analysis.suggested_seed_node_id, "3");

        // Verify KSampler skipped the "randomize" control token properly
        let ksampler_inputs = &api_prompt["3"]["inputs"];
        assert_eq!(ksampler_inputs["seed"], serde_json::json!(123456789));
        assert_eq!(ksampler_inputs["steps"], serde_json::json!(20));
        assert_eq!(ksampler_inputs["sampler_name"], serde_json::json!("euler"));
    }
}
