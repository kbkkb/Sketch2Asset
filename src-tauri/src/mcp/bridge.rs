use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::error::{AppError, AppErrorCode, AppResult};
use crate::state::AppState;

pub const DESKTOP_NOT_RUNNING_MSG: &str = "Sketch2Asset 桌面端未启动，请先让用户打开 Sketch2Asset.exe 后再重试。(Sketch2Asset desktop app is not running. Please ask the user to launch Sketch2Asset.exe first, then retry.)";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInfo {
    pub port: u16,
    pub pid: u32,
    pub token: String,
    pub version: String,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub token: String,
    pub tool: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub ok: bool,
    pub result: Option<Value>,
    pub code: Option<String>,
    pub message: Option<String>,
}

pub fn bridge_file_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("mcp_bridge.json")
}

pub fn cleanup_bridge_file(app_data_dir: &Path) {
    let path = bridge_file_path(app_data_dir);
    let Ok(raw) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(info) = serde_json::from_str::<BridgeInfo>(&raw) else {
        return;
    };
    if info.pid == std::process::id() {
        let _ = fs::remove_file(path);
    }
}

pub fn start_desktop_bridge_server(app: tauri::AppHandle) {
    let app_data_dir = app.state::<AppState>().app_data_dir.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(listener) = TcpListener::bind("127.0.0.1:0").await else {
            return;
        };
        let Ok(addr) = listener.local_addr() else {
            return;
        };
        let token = uuid::Uuid::new_v4().to_string();
        let info = BridgeInfo {
            port: addr.port(),
            pid: std::process::id(),
            token: token.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            started_at: chrono::Utc::now().timestamp_millis(),
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&info) {
            let path = bridge_file_path(&app_data_dir);
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, json_str).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }

        while let Ok((stream, _)) = listener.accept().await {
            let app_clone = app.clone();
            let expected_token = token.clone();
            tokio::spawn(async move {
                handle_bridge_client(stream, app_clone, expected_token).await;
            });
        }
    });
}

async fn write_bridge_response(write_half: &mut tokio::net::tcp::OwnedWriteHalf, resp: &BridgeResponse) {
    if let Ok(mut serialized) = serde_json::to_string(resp) {
        serialized.push('\n');
        let _ = write_half.write_all(serialized.as_bytes()).await;
        let _ = write_half.flush().await;
    }
}

const MAX_BRIDGE_LINE_BYTES: usize = 16 * 1024 * 1024;
const BRIDGE_READ_TIMEOUT_SECS: u64 = 5;

async fn read_bounded_bridge_line<R: tokio::io::AsyncRead + Unpin>(read_half: R) -> Option<String> {
    let mut reader = BufReader::new(read_half.take((MAX_BRIDGE_LINE_BYTES + 1) as u64));
    let mut line = String::new();
    match tokio::time::timeout(
        Duration::from_secs(BRIDGE_READ_TIMEOUT_SECS),
        reader.read_line(&mut line),
    )
    .await
    {
        Ok(Ok(n)) if n > 0 => {
            if line.len() > MAX_BRIDGE_LINE_BYTES && !line.ends_with('\n') {
                None
            } else {
                Some(line)
            }
        }
        _ => None,
    }
}

async fn handle_bridge_client(
    stream: TcpStream,
    app: tauri::AppHandle,
    expected_token: String,
) {
    let (read_half, mut write_half) = stream.into_split();
    let Some(line) = read_bounded_bridge_line(read_half).await else {
        write_bridge_response(
            &mut write_half,
            &BridgeResponse {
                ok: false,
                result: None,
                code: Some("InvalidInput".to_string()),
                message: Some("Bridge request timed out or exceeded size limit".to_string()),
            },
        )
        .await;
        return;
    };

    let req: BridgeRequest = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            write_bridge_response(
                &mut write_half,
                &BridgeResponse {
                    ok: false,
                    result: None,
                    code: Some("InvalidInput".to_string()),
                    message: Some(format!("Invalid bridge request: {}", e)),
                },
            )
            .await;
            return;
        }
    };

    if req.token != expected_token {
        write_bridge_response(
            &mut write_half,
            &BridgeResponse {
                ok: false,
                result: None,
                code: Some("Unauthorized".to_string()),
                message: Some("Invalid desktop bridge session token".to_string()),
            },
        )
        .await;
        return;
    }

    let short_uid = uuid::Uuid::new_v4().to_string();
    let short_prefix = &short_uid[..8];
    let task_id = if req.tool == "s2a_decompose_character" {
        format!("mcp_decompose_{}", short_prefix)
    } else {
        format!("mcp_{}", short_prefix)
    };

    let cancel_rx = {
        let st = app.state::<AppState>();
        st.register_mcp_task(&task_id)
    };

    let exec_res =
        super::execute_tool_with_desktop(&app, &req.tool, &req.args, &task_id, cancel_rx).await;

    app.state::<AppState>().unregister_mcp_task(&task_id);

    let resp = match exec_res {
        Ok(val) => BridgeResponse {
            ok: true,
            result: Some(val),
            code: None,
            message: None,
        },
        Err(err) => BridgeResponse {
            ok: false,
            result: None,
            code: Some(format!("{:?}", err.code)),
            message: Some(err.message),
        },
    };
    write_bridge_response(&mut write_half, &resp).await;
}

pub async fn forward_tool_call(
    app_data_dir: &Path,
    tool: &str,
    args: &Value,
) -> AppResult<Value> {
    let bridge_path = bridge_file_path(app_data_dir);
    let raw = fs::read_to_string(&bridge_path)
        .map_err(|_| AppError::new(AppErrorCode::Network, DESKTOP_NOT_RUNNING_MSG))?;
    let info: BridgeInfo = serde_json::from_str(&raw)
        .map_err(|_| AppError::new(AppErrorCode::Network, DESKTOP_NOT_RUNNING_MSG))?;

    let stream = tokio::time::timeout(
        Duration::from_millis(1500),
        TcpStream::connect(("127.0.0.1", info.port)),
    )
    .await
    .map_err(|_| AppError::new(AppErrorCode::Network, DESKTOP_NOT_RUNNING_MSG))?
    .map_err(|_| AppError::new(AppErrorCode::Network, DESKTOP_NOT_RUNNING_MSG))?;

    let (read_half, mut write_half) = stream.into_split();
    let req = BridgeRequest {
        token: info.token,
        tool: tool.to_string(),
        args: args.clone(),
    };
    let mut line_out = serde_json::to_string(&req)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    line_out.push('\n');

    write_half
        .write_all(line_out.as_bytes())
        .await
        .map_err(|e| AppError::new(AppErrorCode::Network, e.to_string()))?;
    write_half
        .flush()
        .await
        .map_err(|e| AppError::new(AppErrorCode::Network, e.to_string()))?;

    let mut reader = BufReader::new(read_half);
    let mut resp_line = String::new();
    let n = reader
        .read_line(&mut resp_line)
        .await
        .map_err(|_| {
            AppError::new(
                AppErrorCode::Network,
                "Sketch2Asset 桌面端连接已断开，任务可能已中止。",
            )
        })?;
    if n == 0 || resp_line.trim().is_empty() {
        return Err(AppError::new(
            AppErrorCode::Network,
            "Sketch2Asset 桌面端连接已断开，任务可能已中止。",
        ));
    }

    let resp: BridgeResponse = serde_json::from_str(resp_line.trim())
        .map_err(|e| AppError::new(AppErrorCode::Io, format!("Malformed bridge response: {}", e)))?;

    if resp.ok {
        Ok(resp.result.unwrap_or(Value::Null))
    } else {
        let code = match resp.code.as_deref() {
            Some("NotFound") => AppErrorCode::NotFound,
            Some("InvalidInput") => AppErrorCode::InvalidInput,
            Some("MissingApiKey") => AppErrorCode::MissingApiKey,
            Some("ProviderAuth") => AppErrorCode::ProviderAuth,
            Some("ProviderRateLimited") => AppErrorCode::ProviderRateLimited,
            Some("ProviderBadResponse") => AppErrorCode::ProviderBadResponse,
            Some("ProviderUnavailable") => AppErrorCode::ProviderUnavailable,
            Some("Network") => AppErrorCode::Network,
            Some("ImageDecode") => AppErrorCode::ImageDecode,
            Some("MattingFailed") => AppErrorCode::MattingFailed,
            Some("ModelMissing") => AppErrorCode::ModelMissing,
            Some("Io") => AppErrorCode::Io,
            Some("Database") => AppErrorCode::Database,
            Some("Cancelled") => AppErrorCode::Cancelled,
            _ => AppErrorCode::Internal,
        };
        Err(AppError::new(
            code,
            resp.message
                .unwrap_or_else(|| "Desktop bridge execution failed".to_string()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_forward_fails_when_desktop_not_running() {
        let tmp = std::env::temp_dir().join(format!("s2a_bridge_miss_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&tmp);

        let err = forward_tool_call(&tmp, "s2a_status", &json!({}))
            .await
            .expect_err("Expected error when mcp_bridge.json is missing");
        assert!(err.message.contains("Sketch2Asset.exe"));

        // Stale bridge file pointing to an unlistening port must also fail cleanly
        let stale = BridgeInfo {
            port: 1,
            pid: 999999,
            token: "stale-token".to_string(),
            version: "0.2.2".to_string(),
            started_at: 0,
        };
        fs::write(bridge_file_path(&tmp), serde_json::to_string(&stale).unwrap()).unwrap();
        let err2 = forward_tool_call(&tmp, "s2a_status", &json!({}))
            .await
            .expect_err("Expected connection refused on stale bridge");
        assert!(err2.message.contains("Sketch2Asset.exe"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_bridge_tcp_roundtrip_and_token_auth() {
        let tmp = std::env::temp_dir().join(format!("s2a_bridge_ok_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&tmp);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let valid_token = "secret-session-token".to_string();

        let info = BridgeInfo {
            port,
            pid: std::process::id(),
            token: valid_token.clone(),
            version: "0.2.2".to_string(),
            started_at: 123456,
        };
        fs::write(bridge_file_path(&tmp), serde_json::to_string(&info).unwrap()).unwrap();

        let expected = valid_token.clone();
        let server_task = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let exp = expected.clone();
                let (rh, mut wh) = stream.into_split();
                let mut reader = BufReader::new(rh);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                let req: BridgeRequest = serde_json::from_str(line.trim()).unwrap();
                let resp = if req.token == exp {
                    BridgeResponse {
                        ok: true,
                        result: Some(json!({ "echoTool": req.tool, "desktopBridgeConnected": true })),
                        code: None,
                        message: None,
                    }
                } else {
                    BridgeResponse {
                        ok: false,
                        result: None,
                        code: Some("Unauthorized".to_string()),
                        message: Some("Invalid desktop bridge session token".to_string()),
                    }
                };
                write_bridge_response(&mut wh, &resp).await;
            }
        });

        // 1. Valid token succeeds
        let ok_res = forward_tool_call(&tmp, "s2a_status", &json!({})).await.unwrap();
        assert_eq!(ok_res["echoTool"], "s2a_status");
        assert_eq!(ok_res["desktopBridgeConnected"], true);

        // 2. Wrong token gets rejected
        let mut bad_info = info.clone();
        bad_info.token = "wrong-token".to_string();
        fs::write(bridge_file_path(&tmp), serde_json::to_string(&bad_info).unwrap()).unwrap();
        let bad_err = forward_tool_call(&tmp, "s2a_status", &json!({}))
            .await
            .expect_err("Expected token rejection");
        assert!(bad_err.message.contains("Invalid desktop bridge session token"));

        server_task.await.unwrap();
        cleanup_bridge_file(&tmp);
        assert!(!bridge_file_path(&tmp).exists());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_read_bounded_bridge_line_rejects_oversize_and_accepts_valid() {
        let (mut client_ok, server_ok) = tokio::io::duplex(256);
        client_ok.write_all(b"{\"token\":\"t\"}\n").await.unwrap();
        drop(client_ok);
        let parsed = read_bounded_bridge_line(server_ok).await;
        assert_eq!(parsed.as_deref(), Some("{\"token\":\"t\"}\n"));

        // Stream exceeding MAX_BRIDGE_LINE_BYTES without newline must be rejected before buffering unbounded memory
        let (mut client_big, server_big) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            let chunk = vec![b'a'; 64 * 1024];
            for _ in 0..270 {
                if client_big.write_all(&chunk).await.is_err() {
                    break;
                }
            }
        });
        assert!(read_bounded_bridge_line(server_big).await.is_none());
    }
}
