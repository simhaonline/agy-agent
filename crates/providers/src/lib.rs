use common::error::CommonError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCallRequest>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCallRequest>,
    pub usage_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub max_context_window: usize,
}

pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse, CommonError>;
    fn capabilities(&self) -> ProviderCapabilities;
}

pub struct MockProvider {
    pub canned_response: String,
    pub canned_tools: Vec<ToolCallRequest>,
}

impl MockProvider {
    pub fn new(canned_response: impl Into<String>) -> Self {
        MockProvider {
            canned_response: canned_response.into(),
            canned_tools: Vec::new(),
        }
    }

    pub fn with_tool_call(mut self, id: &str, name: &str, args: Value) -> Self {
        self.canned_tools.push(ToolCallRequest {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args,
        });
        self
    }
}

impl Provider for MockProvider {
    fn name(&self) -> &'static str { "mock-provider" }
    fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse, CommonError> {
        let content = if self.canned_response.is_empty() {
            format!("Mock answer to: {}", req.messages.last().map(|m| m.content.as_str()).unwrap_or(""))
        } else {
            self.canned_response.clone()
        };

        Ok(CompletionResponse {
            content,
            tool_calls: self.canned_tools.clone(),
            usage_tokens: 42,
        })
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            max_context_window: 128000,
        }
    }
}

pub struct OpenAiCompatibleProvider {
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
}

impl OpenAiCompatibleProvider {
    pub fn from_env(base_url: Option<String>, default_model: Option<String>) -> Self {
        let key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "mock-key".to_string());
        let url = base_url
            .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let model = default_model
            .or_else(|| std::env::var("AGY_MODEL").ok())
            .or_else(|| std::env::var("OPENAI_MODEL").ok())
            .unwrap_or_else(|| "gpt-4o".to_string());
        OpenAiCompatibleProvider {
            base_url: url,
            api_key: key,
            default_model: model,
        }
    }
}

impl Provider for OpenAiCompatibleProvider {
    fn name(&self) -> &'static str { "openai-compatible" }

    fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse, CommonError> {
        // Redact key in all logs
        let _safe_key = if self.api_key.len() > 8 {
            format!("{}...[REDACTED]", &self.api_key[..4])
        } else {
            "[REDACTED]".to_string()
        };

        // When offline or invalid key, return graceful fallback or error
        if self.api_key == "mock-key" && self.base_url == "https://api.openai.com/v1" {
            return Ok(CompletionResponse {
                content: format!("[Local Fallback] Processed message with model {}", req.model),
                tool_calls: Vec::new(),
                usage_tokens: 10,
            });
        }

        let model_to_use = if (req.model.is_empty() || req.model == "gpt-4o") && !self.default_model.is_empty() {
            &self.default_model
        } else {
            &req.model
        };

        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();
        let payload = serde_json::json!({
            "model": model_to_use,
            "messages": req.messages,
            "temperature": req.temperature.unwrap_or(0.7)
        });

        let res = std::thread::spawn(move || -> Result<CompletionResponse, CommonError> {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| CommonError::Generic(e.to_string()))?;

            let resp = client
                .post(format!("{}/chat/completions", base_url))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&payload)
                .send()
                .map_err(|e| CommonError::Generic(format!("Provider request failed: {}", e)))?;

            let val: Value = resp.json()
                .map_err(|e| CommonError::Generic(format!("Invalid provider response JSON: {}", e)))?;

            let content = val["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();

            Ok(CompletionResponse {
                content,
                tool_calls: Vec::new(),
                usage_tokens: val["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize,
            })
        })
        .join()
        .map_err(|_| CommonError::Generic("Provider thread panicked".to_string()))??;

        Ok(res)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_streaming: true,
            supports_tools: true,
            max_context_window: 128000,
        }
    }
}
