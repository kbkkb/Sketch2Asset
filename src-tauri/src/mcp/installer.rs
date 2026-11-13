use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppErrorCode, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTargetInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub detected: bool,
    pub mcp_installed: bool,
    pub skill_installed: bool,
    pub mcp_config_path: String,
    pub skill_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHubStatus {
    pub mcp_exe_path: String,
    pub app_skill_dir: String,
    pub mcp_json_snippet: String,
    pub codex_toml_snippet: String,
    pub cli_example_snippet: String,
    pub skill_markdown: String,
    pub agents: Vec<AgentTargetInfo>,
}

pub fn resolve_mcp_exe_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("sketch2asset-mcp.exe");
            if candidate.exists() {
                return candidate;
            }
            if let Some(p3) = dir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
                let bin_cand = p3.join("bin").join("sketch2asset-mcp.exe");
                if bin_cand.exists() {
                    return bin_cand;
                }
            }
        }
        return exe;
    }
    PathBuf::from("sketch2asset-mcp.exe")
}

pub fn resolve_mcp_command_and_args() -> (String, Vec<String>) {
    let exe_path = resolve_mcp_exe_path();
    let exe_str = exe_path.to_string_lossy().replace('\\', "/");
    let is_dedicated = exe_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase().contains("mcp"))
        .unwrap_or(false);
    if is_dedicated {
        (exe_str, vec!["--mcp".to_string()])
    } else {
        (exe_str, vec!["--mcp".to_string()])
    }
}

pub fn build_skill_markdown(mcp_exe: &str) -> String {
    format!(
        r#"---
name: sketch2asset
description: >-
  Generate 2D game assets (icons, pixel sprites, characters, tiles, UI), run character turnarounds,
  outfit skinning, sprite sheet animations, and perform local DirectML GPU AI background removal
  with anti-halo protection using Sketch2Asset. Activate when the user needs game art, sprites,
  icons, character sheets, pixel art cleaning, or transparent PNG cutout.
---

# Sketch2Asset — AI Game Asset & Sprite Workstation Skill

You have direct access to **Sketch2Asset** installed on this machine. It reuses the user's configured API keys, project style anchors, and local DirectML GPU ONNX matting models stored in `%AppData%\com.sketch2asset.app`.

## 1. Preferred Method: MCP Tools (If Loaded)

If the `sketch2asset` MCP server is connected in your toolset, call these tools directly:

- `s2a_status`: Inspect active API provider, all configured profiles & models, projects, and installed local ONNX GPU matting models (`birefnet`, `ben2`, `isnet-anime`, `isnet-general-use`). Hot-reloads `settings.json` on every call.
- `s2a_create_project`: Create or update a project folder (`name`, optional `style`, `style_prompt`, `set_active: true`) and sync to `library.db` and `settings.json`.
- `s2a_switch_profile`: Switch active provider profile (`profile_id` by ID or name), active default model (`model`), or active project (`active_project`), persisting immediately to `settings.json`.
- `s2a_generate_asset`: Generate a structured game asset.
  - `prompt`: Detailed subject description.
  - `archetype`: `"item_icon"` | `"pixel_sprite"` | `"character_card"` | `"tile_texture"` | `"ui_element"` | `"free"`.
  - `size`: `"1024x1024"`, `"1536x1024"`, `"1024x1536"`, `"1920x1080"`.
  - `project`, `profile_id`, `model`: Optional overrides for project, provider profile, or model.
  - `auto_remove_bg`: Set `true` for icons, sprites, and characters to automatically run local GPU matting + 3-stage anti-halo (foreground unmixing + choke + 4px color bleed).
  - `output_path`: Optional absolute path inside the current game repository (e.g., `D:/Game/assets/sprites/sword.png`) to save the final PNG directly.
- `s2a_remove_background`: Run local DirectML GPU background removal on any existing image file (`input_path` -> `output_path`), with `model_id` (`"birefnet"`, `"ben2"`, `"isnet-anime"`) and `choke` (`0`..`3`).
- `s2a_generate_character`: Generate character turnaround sheets (`"turnaround"`), outfit skinning (`"outfit"`), animation sprite sheets (`"spritesheet"`), or expression grids (`"expression"`), with optional `project`, `profile_id`, `model`, `anchor_image_path`, `size`, `auto_remove_bg`, and `output_path`.
- `s2a_edit_region`: Modify a rectangular region `[x, y, w, h]` of an asset with optional `profile_id` and `model` overrides.
- `s2a_list_assets`: Query generated assets in the Sketch2Asset library.

## 2. Fallback Method: Direct CLI Execution

If MCP tools are not mounted in the current session, invoke the local CLI binary directly via shell:

```bash
# Check status & available models
"{mcp_exe}" cli status

# Generate a game asset and auto-cutout background directly to a project path
"{mcp_exe}" cli generate --prompt "A glowing crystal mana potion bottle" --archetype item_icon --remove-bg --output "./assets/mana_potion.png"

# Run local GPU AI matting (BiRefNet / BEN2 / ISNet) on any local image
"{mcp_exe}" cli matting --input "./raw.png" --output "./clean.png" --model birefnet --choke 1

# Generate a character turnaround or sprite sheet from an anchor character image
"{mcp_exe}" cli character --mode spritesheet --prompt "6-frame horizontal run cycle, side view" --anchor "./hero.png" --remove-bg --output "./hero_run_sheet.png"
```

## Recommended Game Asset Workflow
1. Check `s2a_status` (or `cli status`) first if unsure which models or projects are configured.
2. When generating props, icons, pixel sprites, or characters intended for Unity / Godot / Unreal / Web games, enable `auto_remove_bg: true` (or `--remove-bg`) so edges are unmixed and 4px color-bled against bilinear filtering halos.
3. Always pass `output_path` (or `--output`) pointing into the user's active workspace asset directory when creating assets for their codebase.
"#
    )
}

fn user_home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

struct AgentSpec {
    id: &'static str,
    name: &'static str,
    vendor: &'static str,
    detect_dir: PathBuf,
    mcp_config_path: PathBuf,
    is_toml: bool,
    skill_file_path: PathBuf,
}

fn get_agent_specs() -> Vec<AgentSpec> {
    let home = user_home_dir();
    vec![
        AgentSpec {
            id: "antigravity",
            name: "Antigravity (AGY)",
            vendor: "Google DeepMind",
            detect_dir: home.join(".gemini"),
            mcp_config_path: home.join(".gemini").join("config").join("mcp_config.json"),
            is_toml: false,
            skill_file_path: home
                .join(".gemini")
                .join("config")
                .join("skills")
                .join("sketch2asset")
                .join("SKILL.md"),
        },
        AgentSpec {
            id: "claude_code",
            name: "Claude Code",
            vendor: "Anthropic",
            detect_dir: home.join(".claude"),
            mcp_config_path: home.join(".claude.json"),
            is_toml: false,
            skill_file_path: home
                .join(".claude")
                .join("skills")
                .join("sketch2asset")
                .join("SKILL.md"),
        },
        AgentSpec {
            id: "codex",
            name: "Codex CLI / App",
            vendor: "OpenAI",
            detect_dir: home.join(".codex"),
            mcp_config_path: home.join(".codex").join("config.toml"),
            is_toml: true,
            skill_file_path: home
                .join(".codex")
                .join("skills")
                .join("sketch2asset")
                .join("SKILL.md"),
        },
        AgentSpec {
            id: "cursor",
            name: "Cursor",
            vendor: "Anysphere",
            detect_dir: home.join(".cursor"),
            mcp_config_path: home.join(".cursor").join("mcp.json"),
            is_toml: false,
            skill_file_path: home
                .join(".cursor")
                .join("skills")
                .join("sketch2asset")
                .join("SKILL.md"),
        },
    ]
}

fn check_json_mcp_installed(path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    val.get("mcpServers")
        .and_then(|m| m.get("sketch2asset"))
        .is_some()
}

fn check_toml_mcp_installed(path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    raw.contains("[mcp_servers.sketch2asset]")
}

pub fn get_hub_status(app_data_dir: &Path) -> AppResult<AgentHubStatus> {
    let (cmd, args) = resolve_mcp_command_and_args();
    let skill_md = build_skill_markdown(&cmd);

    // Ensure local canonical copy in %AppData%\com.sketch2asset.app\agent_skill\sketch2asset\SKILL.md
    let app_skill_dir = app_data_dir.join("agent_skill").join("sketch2asset");
    let _ = fs::create_dir_all(&app_skill_dir);
    let _ = fs::write(app_skill_dir.join("SKILL.md"), &skill_md);

    let mcp_json_obj = json!({
        "mcpServers": {
            "sketch2asset": {
                "command": cmd,
                "args": args
            }
        }
    });
    let mcp_json_snippet =
        serde_json::to_string_pretty(&mcp_json_obj).unwrap_or_else(|_| "{}".to_string());

    let codex_toml_snippet = format!(
        "[mcp_servers.sketch2asset]\ncommand = \"{}\"\nargs = [\"--mcp\"]\n",
        cmd
    );

    let cli_example_snippet = format!(
        "\"{}\" cli generate --prompt \"A golden treasure chest icon\" --archetype item_icon --remove-bg --output \"./chest.png\"",
        cmd
    );

    let mut agents = Vec::new();
    for spec in get_agent_specs() {
        let detected = spec.detect_dir.exists() || spec.mcp_config_path.exists();
        let mcp_installed = if spec.is_toml {
            check_toml_mcp_installed(&spec.mcp_config_path)
        } else {
            check_json_mcp_installed(&spec.mcp_config_path)
        };
        let skill_installed = spec.skill_file_path.exists();

        agents.push(AgentTargetInfo {
            id: spec.id.to_string(),
            name: spec.name.to_string(),
            vendor: spec.vendor.to_string(),
            detected,
            mcp_installed,
            skill_installed,
            mcp_config_path: spec.mcp_config_path.to_string_lossy().replace('\\', "/"),
            skill_path: spec.skill_file_path.to_string_lossy().replace('\\', "/"),
        });
    }

    Ok(AgentHubStatus {
        mcp_exe_path: cmd,
        app_skill_dir: app_skill_dir.to_string_lossy().replace('\\', "/"),
        mcp_json_snippet,
        codex_toml_snippet,
        cli_example_snippet,
        skill_markdown: skill_md,
        agents,
    })
}

fn upsert_json_mcp_config(path: &Path, cmd: &str, args: &[String]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut root: Value = if path.exists() {
        let raw = fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }
    let root_obj = root.as_object_mut().unwrap();
    if !root_obj.contains_key("mcpServers") || !root_obj["mcpServers"].is_object() {
        root_obj.insert("mcpServers".to_string(), json!({}));
    }
    if let Some(servers) = root_obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.insert(
            "sketch2asset".to_string(),
            json!({
                "command": cmd,
                "args": args
            }),
        );
    }

    let formatted = serde_json::to_string_pretty(&root)?;
    fs::write(path, formatted)?;
    Ok(())
}

fn remove_json_mcp_config(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Ok(());
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(());
    };
    let Ok(mut root) = serde_json::from_str::<Value>(&raw) else {
        return Ok(());
    };
    if let Some(servers) = root.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove("sketch2asset");
        let formatted = serde_json::to_string_pretty(&root)?;
        fs::write(path, formatted)?;
    }
    Ok(())
}

fn strip_toml_section(raw: &str, header: &str) -> String {
    let mut out = Vec::new();
    let mut skipping = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == header {
            skipping = true;
            continue;
        }
        if skipping && trimmed.starts_with('[') && trimmed.ends_with(']') {
            skipping = false;
        }
        if !skipping {
            out.push(line);
        }
    }
    out.join("\n")
}

fn upsert_toml_mcp_config(path: &Path, cmd: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    };
    let cleaned = strip_toml_section(&existing, "[mcp_servers.sketch2asset]");
    let mut final_toml = cleaned.trim_end().to_string();
    if !final_toml.is_empty() {
        final_toml.push_str("\n\n");
    }
    final_toml.push_str(&format!(
        "[mcp_servers.sketch2asset]\ncommand = \"{}\"\nargs = [\"--mcp\"]\n",
        cmd
    ));
    fs::write(path, final_toml)?;
    Ok(())
}

fn remove_toml_mcp_config(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Ok(());
    }
    if let Ok(existing) = fs::read_to_string(path) {
        let cleaned = strip_toml_section(&existing, "[mcp_servers.sketch2asset]");
        fs::write(path, format!("{}\n", cleaned.trim_end()))?;
    }
    Ok(())
}

pub fn install_for_agent(app_data_dir: &Path, agent_id: &str) -> AppResult<AgentHubStatus> {
    let (cmd, args) = resolve_mcp_command_and_args();
    let skill_md = build_skill_markdown(&cmd);
    let specs = get_agent_specs();

    let mut matched = false;
    for spec in specs {
        if agent_id == "all" || spec.id == agent_id {
            matched = true;
            if spec.is_toml {
                upsert_toml_mcp_config(&spec.mcp_config_path, &cmd)?;
            } else {
                upsert_json_mcp_config(&spec.mcp_config_path, &cmd, &args)?;
            }
            if let Some(skill_parent) = spec.skill_file_path.parent() {
                fs::create_dir_all(skill_parent)?;
                fs::write(&spec.skill_file_path, &skill_md)?;
            }
        }
    }

    if !matched {
        return Err(AppError::new(
            AppErrorCode::NotFound,
            format!("Unknown agent id: {}", agent_id),
        ));
    }

    get_hub_status(app_data_dir)
}

pub fn uninstall_for_agent(app_data_dir: &Path, agent_id: &str) -> AppResult<AgentHubStatus> {
    let specs = get_agent_specs();
    for spec in specs {
        if agent_id == "all" || spec.id == agent_id {
            if spec.is_toml {
                remove_toml_mcp_config(&spec.mcp_config_path)?;
            } else {
                remove_json_mcp_config(&spec.mcp_config_path)?;
            }
            if spec.skill_file_path.exists() {
                let _ = fs::remove_file(&spec.skill_file_path);
                if let Some(parent) = spec.skill_file_path.parent() {
                    let _ = fs::remove_dir(parent);
                }
            }
        }
    }
    get_hub_status(app_data_dir)
}

pub fn export_skill_bundle(app_data_dir: &Path, reveal: bool) -> AppResult<String> {
    let status = get_hub_status(app_data_dir)?;
    let base_dir = dirs::download_dir().unwrap_or_else(|| app_data_dir.join("_exports"));
    let bundle_dir = base_dir.join("sketch2asset-agent-bundle");
    let skill_sub = bundle_dir.join("skills").join("sketch2asset");
    fs::create_dir_all(&skill_sub)?;

    fs::write(skill_sub.join("SKILL.md"), &status.skill_markdown)?;
    fs::write(bundle_dir.join("mcp_config.json"), &status.mcp_json_snippet)?;
    fs::write(bundle_dir.join("codex_mcp.toml"), &status.codex_toml_snippet)?;

    if reveal {
        let win_path = bundle_dir.to_string_lossy().replace('/', "\\");
        let _ = std::process::Command::new("explorer.exe")
            .arg(&win_path)
            .spawn();
    }

    Ok(bundle_dir.to_string_lossy().replace('\\', "/"))
}
