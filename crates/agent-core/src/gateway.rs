use crate::account::AccountStore;
use crate::audit::AuditLogger;
use chrono::Utc;
use common::error::CommonError;
use providers::{ChatMessage, CompletionRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
    pub workspace: PathBuf,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        GatewayConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            workspace: PathBuf::from("."),
        }
    }
}

pub struct GatewayServer {
    pub config: GatewayConfig,
    pub accounts: Arc<RwLock<AccountStore>>,
    pub audit: Arc<AuditLogger>,
}

impl GatewayServer {
    pub fn new(config: GatewayConfig, accounts: AccountStore, audit: Arc<AuditLogger>) -> Self {
        GatewayServer {
            config,
            accounts: Arc::new(RwLock::new(accounts)),
            audit,
        }
    }

    pub async fn run(&self) -> Result<(), CommonError> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| CommonError::Generic(format!("Invalid gateway address: {}", e)))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| CommonError::Generic(format!("Failed to bind gateway on {}: {}", addr, e)))?;

        println!("AGY Universal API Gateway listening on http://{}", addr);
        println!("Endpoints:");
        println!("  - Web Dashboard: GET http://{}/dashboard", addr);
        println!("  - OpenAI: POST http://{}/v1/chat/completions", addr);
        println!("  - Anthropic Claude: POST http://{}/v1/messages", addr);
        println!("  - Google Gemini: POST http://{}/v1beta/models/gemini-1.5-pro:generateContent", addr);
        println!("  - Models list: GET http://{}/v1/models", addr);
        println!("  - Health check: GET http://{}/health", addr);
        println!("  - Telemetry: GET http://{}/status", addr);

        let config_arc = Arc::new(self.config.clone());

        loop {
            match listener.accept().await {
                Ok((stream, client_addr)) => {
                    let accounts = Arc::clone(&self.accounts);
                    let audit = Arc::clone(&self.audit);
                    let config = Arc::clone(&config_arc);
                    tokio::spawn(async move {
                        if let Err(e) = handle_http_stream(stream, accounts, audit, config, client_addr).await {
                            eprintln!("Error handling connection from {}: {}", client_addr, e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept incoming gateway connection: {}", e);
                }
            }
        }
    }
}

async fn handle_http_stream(
    mut stream: TcpStream,
    accounts: Arc<RwLock<AccountStore>>,
    audit: Arc<AuditLogger>,
    config: Arc<GatewayConfig>,
    _client_addr: SocketAddr,
) -> Result<(), CommonError> {
    let mut buffer = Vec::with_capacity(8192);
    let mut temp = [0u8; 4096];
    let mut header_end = None;

    // Read headers until \r\n\r\n
    while header_end.is_none() {
        let n = stream
            .read(&mut temp)
            .await
            .map_err(|e| CommonError::Generic(e.to_string()))?;
        if n == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&temp[..n]);

        if let Some(pos) = find_subsequence(&buffer, b"\r\n\r\n") {
            header_end = Some(pos);
            break;
        }
    }

    let header_pos = match header_end {
        Some(pos) => pos,
        None => return Ok(()),
    };

    let header_bytes = &buffer[..header_pos];
    let header_str = String::from_utf8_lossy(header_bytes);
    let mut lines = header_str.lines();

    let request_line = match lines.next() {
        Some(line) => line,
        None => return Ok(()),
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    // Parse Content-Length
    let mut content_length: usize = 0;
    for line in lines {
        if line.to_ascii_lowercase().starts_with("content-length:") {
            if let Some(val_str) = line.split(':').nth(1) {
                content_length = val_str.trim().parse::<usize>().unwrap_or(0);
            }
        }
    }

    // Read remaining body if needed
    let body_start = header_pos + 4;
    let mut body_bytes = buffer[body_start..].to_vec();
    while body_bytes.len() < content_length {
        let n = stream
            .read(&mut temp)
            .await
            .map_err(|e| CommonError::Generic(e.to_string()))?;
        if n == 0 {
            break;
        }
        body_bytes.extend_from_slice(&temp[..n]);
    }
    body_bytes.truncate(content_length);

    // Handle CORS Preflight
    if method == "OPTIONS" {
        let resp = "HTTP/1.1 204 No Content\r\n\
                    Access-Control-Allow-Origin: *\r\n\
                    Access-Control-Allow-Methods: GET, POST, OPTIONS, HEAD\r\n\
                    Access-Control-Allow-Headers: Content-Type, Authorization, x-api-key, anthropic-version\r\n\
                    Content-Length: 0\r\n\
                    Connection: close\r\n\r\n";
        stream
            .write_all(resp.as_bytes())
            .await
            .map_err(|e| CommonError::Generic(e.to_string()))?;
        return Ok(());
    }

    // Serve HTML Web Dashboard
    if (path == "/" || path == "/dashboard") && method == "GET" {
        let html = include_str!("dashboard.html");
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            html.len(),
            html
        );
        stream
            .write_all(resp.as_bytes())
            .await
            .map_err(|e| CommonError::Generic(e.to_string()))?;
        return Ok(());
    }

    // Route requests
    let (status_code, body_json) = if path == "/health" {
        (200, json!({ "status": "ok", "service": "agy-gateway", "version": "0.1.0" }))
    } else if path == "/api/accounts" && method == "GET" {
        let store = accounts.read().await;
        (200, json!(store.rank_accounts()))
    } else if path == "/api/accounts" && method == "POST" {
        let val: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
        let name = val["name"].as_str().unwrap_or("Account").to_string();
        let provider = val["provider"].as_str().unwrap_or("openai").to_string();
        let key = val["key"].as_str().unwrap_or("").to_string();
        let model = val["model"].as_str().map(|s| s.to_string());
        let url = val["url"].as_str().map(|s| s.to_string());
        let quota = val["quota"].as_u64();
        let email = val["email"].as_str().map(|s| s.to_string());
        let tier = val["plan_tier"].as_str().map(|s| s.to_string());
        let mut profile = crate::account::AccountProfile::new(name, provider, key, url, model, quota);
        if let Some(em) = email {
            profile.email = Some(em);
        }
        if let Some(t) = tier {
            profile.plan_tier = Some(t);
        }
        let mut store = accounts.write().await;
        match store.add_account(profile) {
            Ok(_) => {
                let acc_path = crate::account::AccountStore::default_path(&config.workspace);
                let _ = store.save_to_path(&acc_path);
                (200, json!({ "status": "ok", "message": "Account registered" }))
            }
            Err(e) => (400, json!({ "error": e.to_string() })),
        }
    } else if (path == "/api/accounts/switch" || path == "/api/auth/switch") && method == "POST" {
        let val: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
        let id_or_name = val["id_or_name"].as_str().or_else(|| val["name"].as_str()).unwrap_or("");
        let mut store = accounts.write().await;
        match crate::auth::AuthCredentialManager::apply_switch(&mut store, id_or_name, crate::auth::TargetMode::Both, &config.workspace) {
            Ok(report) => {
                let _ = audit.log("gateway", "gateway", "auth_switch", json!({ "account": id_or_name, "targets": report.updated_paths.len(), "backups": report.backups_created.len() }));
                (200, json!({ "status": "ok", "message": format!("Switched to {}", id_or_name), "report": report }))
            }
            Err(e) => (400, json!({ "error": e.to_string() })),
        }
    } else if path == "/api/auth/rotate" && method == "POST" {
        let val: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
        let threshold = val["threshold"].as_f64().unwrap_or(90.0);
        let mut store = accounts.write().await;
        match store.auto_rotate(threshold) {
            Ok(Some(rot)) => {
                let target_name = rot.to.clone();
                let _ = crate::auth::AuthCredentialManager::apply_switch(&mut store, &target_name, crate::auth::TargetMode::Both, &config.workspace);
                let _ = audit.log("gateway", "gateway", "auth_auto_rotate", json!({ "from": rot.from, "to": rot.to, "reason": rot.reason }));
                (200, json!({ "status": "ok", "rotated": true, "rotation": rot }))
            }
            Ok(None) => (200, json!({ "status": "ok", "rotated": false, "message": "Active account is healthy, no rotation needed" })),
            Err(e) => (400, json!({ "error": e.to_string() })),
        }
    } else if path == "/api/auth/sync" && method == "POST" {
        let mut store = accounts.write().await;
        match crate::auth::AuthCredentialManager::sync_with_cli(&mut store, &config.workspace) {
            Ok(report) => {
                let _ = audit.log("gateway", "gateway", "auth_sync", json!({ "action": report.action_taken }));
                (200, json!({ "status": "ok", "report": report }))
            }
            Err(e) => (400, json!({ "error": e.to_string() })),
        }
    } else if path == "/api/auth/backups" && method == "GET" {
        let backups = crate::auth::AuthCredentialManager::list_backups(&config.workspace);
        (200, json!(backups))
    } else if path == "/api/accounts/delete" && method == "POST" {
        let val: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
        let id_or_name = val["id_or_name"].as_str().unwrap_or("");
        let mut store = accounts.write().await;
        match store.remove_account(id_or_name) {
            Ok(rem) => {
                let acc_path = crate::account::AccountStore::default_path(&config.workspace);
                let _ = store.save_to_path(&acc_path);
                (200, json!({ "status": "ok", "message": format!("Removed {}", rem.name) }))
            }
            Err(e) => (400, json!({ "error": e.to_string() })),
        }
    } else if path == "/api/client" && method == "GET" {
        let report = crate::manager::ClientManager::inspect_environment(Some(config.port));
        (200, json!(report))
    } else if path == "/api/audit" && method == "GET" {
        let events = audit.read_events().unwrap_or_default();
        (200, json!(events))
    } else if path == "/api/proxy/stats" && method == "GET" {
        let store = accounts.read().await;
        let accounts = store.list_accounts();
        let total_requests: u64 = accounts.iter().map(|a| a.request_count).sum();
        let total_errors: u64 = accounts.iter().map(|a| a.error_count).sum();
        let total_tokens: u64 = accounts.iter().map(|a| a.quota_used).sum();
        let cooling_count = accounts.iter().filter(|a| a.is_cooling()).count();
        let active_name = store.get_active_account().map(|a| a.name.clone());
        (
            200,
            json!({
                "service": "agy-proxy",
                "port": config.port,
                "host": config.host,
                "total_accounts": accounts.len(),
                "active_account": active_name,
                "cooling_accounts": cooling_count,
                "total_requests": total_requests,
                "total_errors": total_errors,
                "total_tokens": total_tokens,
                "timestamp": Utc::now()
            }),
        )
    } else if path == "/api/auth/restore" && method == "POST" {
        let val: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
        let file_path = val["file"].as_str().unwrap_or("");
        let target_str = val["target"].as_str().unwrap_or("both");
        let target_mode = target_str.parse::<crate::auth::TargetMode>().unwrap_or_default();
        match crate::auth::AuthCredentialManager::restore_backup(std::path::Path::new(file_path), target_mode) {
            Ok(report) => {
                let _ = audit.log("gateway", "gateway", "auth_restore", json!({ "file": file_path, "targets": report.updated_paths.len() }));
                (200, json!({ "status": "ok", "message": "Restored backup successfully", "report": report }))
            }
            Err(e) => (400, json!({ "error": e.to_string() })),
        }
    } else if path == "/status" || path == "/v1/status" {
        let store = accounts.read().await;
        let total = store.list_accounts().len();
        let cooling = store.list_accounts().iter().filter(|a| a.is_cooling()).count();
        let active = store.get_active_account().map(|a| a.name.clone());
        (
            200,
            json!({
                "status": "active",
                "accounts_total": total,
                "accounts_cooling": cooling,
                "active_account": active,
                "timestamp": Utc::now()
            }),
        )
    } else if path == "/v1/models" {
        (
            200,
            json!({
                "object": "list",
                "data": [
                    { "id": "gemini-4-argon", "object": "model", "owned_by": "google", "description": "Gemini 4 Argon Advanced Reasoning Model (Argon Bridge)" },
                    { "id": "gemini-2.0-flash-thinking", "object": "model", "owned_by": "google", "description": "Gemini 2.0 Flash Thinking Deep-Reasoning Model" },
                    { "id": "gemini-2.0-flash", "object": "model", "owned_by": "google" },
                    { "id": "claude-3-5-sonnet", "object": "model", "owned_by": "anthropic" },
                    { "id": "claude-3-opus", "object": "model", "owned_by": "anthropic" },
                    { "id": "gpt-4o", "object": "model", "owned_by": "openai" },
                    { "id": "gpt-4-turbo", "object": "model", "owned_by": "openai" },
                    { "id": "deepseek-coder", "object": "model", "owned_by": "deepseek" },
                    { "id": "agy-default", "object": "model", "owned_by": "agy" }
                ]
            }),
        )
    } else if path == "/v1/chat/completions" && method == "POST" {
        handle_openai_chat(body_bytes, accounts, audit).await
    } else if path == "/v1/messages" && method == "POST" {
        handle_claude_messages(body_bytes, accounts, audit).await
    } else if (path.starts_with("/v1beta/models/") || path.starts_with("/v1/models/"))
        && path.contains("generateContent")
        && method == "POST"
    {
        handle_gemini_generate(body_bytes, accounts, audit).await
    } else {
        (
            404,
            json!({
                "error": {
                    "message": format!("Route not found: {} {}", method, path),
                    "type": "invalid_request_error"
                }
            }),
        )
    };

    let body_str = serde_json::to_string(&body_json).unwrap_or_else(|_| "{}".to_string());
    let resp = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: application/json\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{}",
        status_code,
        status_text(status_code),
        body_str.len(),
        body_str
    );

    stream
        .write_all(resp.as_bytes())
        .await
        .map_err(|e| CommonError::Generic(e.to_string()))?;
    Ok(())
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Unknown",
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

pub fn translate_openai_request(val: &Value) -> Result<(String, Vec<ChatMessage>), CommonError> {
    let model = val["model"].as_str().unwrap_or("gpt-4o").to_string();
    let messages_arr = val["messages"]
        .as_array()
        .ok_or_else(|| CommonError::Generic("Missing messages array".to_string()))?;

    let mut messages = Vec::new();
    for m in messages_arr {
        let role = m["role"].as_str().unwrap_or("user").to_string();
        let content = if let Some(txt) = m["content"].as_str() {
            txt.to_string()
        } else if let Some(parts) = m["content"].as_array() {
            let texts: Vec<String> = parts
                .iter()
                .filter_map(|p| p["text"].as_str().map(|s| s.to_string()))
                .collect();
            texts.join("\n")
        } else {
            String::new()
        };
        messages.push(ChatMessage {
            role,
            content,
            tool_calls: None,
        });
    }

    Ok((model, messages))
}

pub fn format_openai_response(id: &str, model: &str, content: &str, tokens: usize) -> Value {
    json!({
        "id": id,
        "object": "chat.completion",
        "created": Utc::now().timestamp(),
        "model": model,
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": (tokens / 2).max(1),
            "completion_tokens": (tokens / 2).max(1),
            "total_tokens": tokens.max(2)
        }
    })
}

pub fn translate_claude_request(val: &Value) -> Result<(String, Vec<ChatMessage>), CommonError> {
    let model = val["model"].as_str().unwrap_or("claude-3-5-sonnet").to_string();
    let mut messages = Vec::new();

    // Anthropic top-level system parameter
    if let Some(sys) = val["system"].as_str() {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: sys.to_string(),
            tool_calls: None,
        });
    }

    let messages_arr = val["messages"]
        .as_array()
        .ok_or_else(|| CommonError::Generic("Missing messages array".to_string()))?;

    for m in messages_arr {
        let role = m["role"].as_str().unwrap_or("user").to_string();
        let content = if let Some(txt) = m["content"].as_str() {
            txt.to_string()
        } else if let Some(parts) = m["content"].as_array() {
            let texts: Vec<String> = parts
                .iter()
                .filter_map(|p| p["text"].as_str().map(|s| s.to_string()))
                .collect();
            texts.join("\n")
        } else {
            String::new()
        };
        messages.push(ChatMessage {
            role,
            content,
            tool_calls: None,
        });
    }

    Ok((model, messages))
}

pub fn format_claude_response(id: &str, model: &str, content: &str, tokens: usize) -> Value {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [
            {
                "type": "text",
                "text": content
            }
        ],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": (tokens / 2).max(1),
            "output_tokens": (tokens / 2).max(1)
        }
    })
}

pub fn translate_gemini_request(val: &Value) -> Result<(String, Vec<ChatMessage>), CommonError> {
    let contents = val["contents"]
        .as_array()
        .ok_or_else(|| CommonError::Generic("Missing contents array in Gemini request".to_string()))?;

    let mut messages = Vec::new();
    for item in contents {
        let role = match item["role"].as_str().unwrap_or("user") {
            "model" => "assistant".to_string(),
            other => other.to_string(),
        };

        let mut text_parts = Vec::new();
        if let Some(parts) = item["parts"].as_array() {
            for part in parts {
                if let Some(txt) = part["text"].as_str() {
                    text_parts.push(txt);
                }
            }
        }
        messages.push(ChatMessage {
            role,
            content: text_parts.join("\n"),
            tool_calls: None,
        });
    }

    Ok(("gemini-1.5-pro".to_string(), messages))
}

pub fn format_gemini_response(content: &str, tokens: usize) -> Value {
    json!({
        "candidates": [
            {
                "content": {
                    "parts": [
                        { "text": content }
                    ],
                    "role": "model"
                },
                "finishReason": "STOP",
                "index": 0
            }
        ],
        "usageMetadata": {
            "promptTokenCount": (tokens / 2).max(1),
            "candidatesTokenCount": (tokens / 2).max(1),
            "totalTokenCount": tokens.max(2)
        }
    })
}

async fn execute_with_account_failover(
    req: &CompletionRequest,
    accounts: &Arc<RwLock<AccountStore>>,
    audit: &Arc<AuditLogger>,
) -> Result<(String, usize), (u16, String)> {
    // Retry loop over available healthy accounts
    let max_retries = 3;
    for _ in 0..max_retries {
        let (account_name, base_url, api_key, model_choice) = {
            let store = accounts.read().await;
            let acc = match store.get_next_available_account() {
                Some(a) => a.clone(),
                None => {
                    // Check if any accounts exist at all
                    if store.list_accounts().is_empty() {
                        // Fallback to local default / mock
                        return Ok((format!("[AGY Gateway Response] Echo: {}", req.messages.last().map(|m| m.content.as_str()).unwrap_or("")), 42));
                    } else {
                        return Err((429, "All configured accounts are currently cooling or exhausted".to_string()));
                    }
                }
            };
            (acc.name, acc.base_url, acc.api_key, acc.model)
        };

        let target_model = model_choice.unwrap_or_else(|| req.model.clone());
        let effective_req = CompletionRequest {
            model: target_model,
            messages: req.messages.clone(),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
        };

        if api_key == "mock-key" || api_key.starts_with("mock-") {
            let mut store = accounts.write().await;
            let _ = store.record_usage(&account_name, 25, false);
            return Ok((format!("[AGY Gateway Response] Processed message with model {}", effective_req.model), 25));
        }

        let base_url_str = base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let target_url = if base_url_str.ends_with("/chat/completions") {
            base_url_str
        } else {
            format!("{}/chat/completions", base_url_str.trim_end_matches('/'))
        };

        let client = reqwest::Client::new();
        let payload = json!({
            "model": effective_req.model,
            "messages": effective_req.messages,
            "temperature": effective_req.temperature.unwrap_or(0.7),
        });

        match client
            .post(&target_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if status.as_u16() == 429 {
                    println!("[AGY Gateway] Rate limit 429 detected on account '{}'. Cooling for 60s and failing over...", account_name);
                    let mut store = accounts.write().await;
                    let _ = store.mark_cooldown(&account_name, 60);
                    let _ = store.record_usage(&account_name, 0, true);
                    let _ = audit.log("gateway", "Gateway", "gateway_rate_limited_429", json!({ "account": account_name }));
                    continue;
                }

                if !status.is_success() {
                    let err_body = resp.text().await.unwrap_or_default();
                    let mut store = accounts.write().await;
                    let _ = store.record_usage(&account_name, 0, true);
                    let _ = audit.log("gateway", "Gateway", "gateway_request_error", json!({ "account": account_name, "error": err_body }));
                    return Err((status.as_u16(), format!("Provider upstream error: {}", err_body)));
                }

                match resp.json::<Value>().await {
                    Ok(val) => {
                        let content = val["choices"][0]["message"]["content"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let tokens = val["usage"]["total_tokens"].as_u64().unwrap_or(30) as usize;

                        let mut store = accounts.write().await;
                        let _ = store.record_usage(&account_name, tokens as u64, false);
                        let _ = audit.log("gateway", "Gateway", "gateway_request_success", json!({ "account": account_name, "tokens": tokens }));
                        return Ok((content, tokens));
                    }
                    Err(e) => {
                        return Err((502, format!("Invalid upstream JSON: {}", e)));
                    }
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                let is_429 = err_msg.contains("429");
                let mut store = accounts.write().await;
                let _ = store.record_usage(&account_name, 0, true);
                if is_429 {
                    println!("[AGY Gateway] Rate limit 429 detected on account '{}'. Cooling for 60s and failing over...", account_name);
                    let _ = store.mark_cooldown(&account_name, 60);
                    continue;
                } else {
                    return Err((502, format!("Failed to reach upstream provider: {}", err_msg)));
                }
            }
        }
    }

    Err((429, "Exhausted retry attempts across available accounts".to_string()))
}

async fn handle_openai_chat(
    body_bytes: Vec<u8>,
    accounts: Arc<RwLock<AccountStore>>,
    audit: Arc<AuditLogger>,
) -> (u16, Value) {
    let val: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            return (
                400,
                json!({ "error": { "message": format!("Invalid JSON: {}", e), "type": "invalid_request_error" } }),
            )
        }
    };

    let (model, messages) = match translate_openai_request(&val) {
        Ok(res) => res,
        Err(e) => {
            return (
                400,
                json!({ "error": { "message": e.to_string(), "type": "invalid_request_error" } }),
            )
        }
    };

    let req = CompletionRequest {
        model: model.clone(),
        messages,
        temperature: val["temperature"].as_f64().map(|f| f as f32),
        max_tokens: val["max_tokens"].as_u64().map(|u| u as usize),
    };

    match execute_with_account_failover(&req, &accounts, &audit).await {
        Ok((content, tokens)) => {
            let id = format!("chatcmpl-{}", Uuid::new_v4());
            (200, format_openai_response(&id, &model, &content, tokens))
        }
        Err((status, msg)) => (
            status,
            json!({ "error": { "message": msg, "type": "api_error" } }),
        ),
    }
}

async fn handle_claude_messages(
    body_bytes: Vec<u8>,
    accounts: Arc<RwLock<AccountStore>>,
    audit: Arc<AuditLogger>,
) -> (u16, Value) {
    let val: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            return (
                400,
                json!({ "error": { "message": format!("Invalid JSON: {}", e), "type": "invalid_request_error" } }),
            )
        }
    };

    let (model, messages) = match translate_claude_request(&val) {
        Ok(res) => res,
        Err(e) => {
            return (
                400,
                json!({ "error": { "message": e.to_string(), "type": "invalid_request_error" } }),
            )
        }
    };

    let req = CompletionRequest {
        model: model.clone(),
        messages,
        temperature: val["temperature"].as_f64().map(|f| f as f32),
        max_tokens: val["max_tokens"].as_u64().map(|u| u as usize),
    };

    match execute_with_account_failover(&req, &accounts, &audit).await {
        Ok((content, tokens)) => {
            let id = format!("msg_{}", Uuid::new_v4());
            (200, format_claude_response(&id, &model, &content, tokens))
        }
        Err((status, msg)) => (
            status,
            json!({ "error": { "message": msg, "type": "api_error" } }),
        ),
    }
}

async fn handle_gemini_generate(
    body_bytes: Vec<u8>,
    accounts: Arc<RwLock<AccountStore>>,
    audit: Arc<AuditLogger>,
) -> (u16, Value) {
    let val: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            return (
                400,
                json!({ "error": { "message": format!("Invalid JSON: {}", e), "type": "invalid_request_error" } }),
            )
        }
    };

    let (model, messages) = match translate_gemini_request(&val) {
        Ok(res) => res,
        Err(e) => {
            return (
                400,
                json!({ "error": { "message": e.to_string(), "type": "invalid_request_error" } }),
            )
        }
    };

    let req = CompletionRequest {
        model,
        messages,
        temperature: None,
        max_tokens: None,
    };

    match execute_with_account_failover(&req, &accounts, &audit).await {
        Ok((content, tokens)) => (200, format_gemini_response(&content, tokens)),
        Err((status, msg)) => (
            status,
            json!({ "error": { "message": msg, "type": "api_error" } }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_translation() {
        let req_json = json!({
            "model": "gpt-4o",
            "messages": [
                { "role": "system", "content": "You are helpful." },
                { "role": "user", "content": "Hello world" }
            ],
            "temperature": 0.5
        });

        let (model, messages) = translate_openai_request(&req_json).unwrap();
        assert_eq!(model, "gpt-4o");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "Hello world");

        let resp_json = format_openai_response("chatcmpl-123", "gpt-4o", "Hi there!", 20);
        assert_eq!(resp_json["id"], "chatcmpl-123");
        assert_eq!(resp_json["choices"][0]["message"]["content"], "Hi there!");
        assert_eq!(resp_json["usage"]["total_tokens"], 20);
    }

    #[test]
    fn test_claude_translation() {
        let req_json = json!({
            "model": "claude-3-5-sonnet",
            "system": "Anthropic system context",
            "messages": [
                { "role": "user", "content": "Analyze code" }
            ]
        });

        let (model, messages) = translate_claude_request(&req_json).unwrap();
        assert_eq!(model, "claude-3-5-sonnet");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "Anthropic system context");
        assert_eq!(messages[1].role, "user");

        let resp_json = format_claude_response("msg_999", "claude-3-5-sonnet", "Review ok", 30);
        assert_eq!(resp_json["type"], "message");
        assert_eq!(resp_json["content"][0]["text"], "Review ok");
    }

    #[test]
    fn test_gemini_translation() {
        let req_json = json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": "What is AGY?" }
                    ]
                }
            ]
        });

        let (model, messages) = translate_gemini_request(&req_json).unwrap();
        assert_eq!(model, "gemini-1.5-pro");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "What is AGY?");

        let resp_json = format_gemini_response("AGY is autonomous", 18);
        assert_eq!(resp_json["candidates"][0]["content"]["parts"][0]["text"], "AGY is autonomous");
        assert_eq!(resp_json["usageMetadata"]["totalTokenCount"], 18);
    }
}
