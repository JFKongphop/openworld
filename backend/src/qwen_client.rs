/*!
Qwen LLM Client — inference engine for all OpenWorld agents.

Uses the Qwen (Alibaba Cloud Model Studio) OpenAI-compatible
chat completions API to power all agent reasoning: itinerary planning,
search result ranking, recovery decisions, and travel tips generation.

Env vars:
  QWEN_ENDPOINT  — chat completions URL
                   e.g. https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions
  QWEN_MODEL     — model name (default: qwen-max)
  QWEN_API_KEY   — DashScope API key (sk-...)
*/

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct ChatMessage {
  role: String,
  content: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
  model: String,
  messages: Vec<ChatMessage>,
  #[serde(skip_serializing_if = "Option::is_none")]
  max_tokens: Option<u32>,
  /// Disable chain-of-thought for qwen3.x models (keeps latency low)
  #[serde(skip_serializing_if = "Option::is_none")]
  enable_thinking: Option<bool>,
}

#[derive(Deserialize)]
struct ChatChoice {
  message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
  choices: Vec<ChatChoice>,
}

// ─── Client ───────────────────────────────────────────────────────────────────

/// Qwen LLM client — reasoning engine for all travel agents
#[derive(Clone)]
pub struct QwenClient {
  endpoint: String,
  model: String,
  api_key: Option<String>,
  http: Client,
}

impl QwenClient {
  /// Create client without API key
  pub fn new(endpoint: String, model: String) -> Self {
    Self {
      endpoint,
      model,
      api_key: None,
      http: Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap_or_default(),
    }
  }

  /// Create client with Bearer token (DashScope sk-... key)
  pub fn with_api_key(endpoint: String, model: String, api_key: String) -> Self {
    Self {
      endpoint,
      model,
      api_key: Some(api_key),
      http: Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap_or_default(),
    }
  }

  /// Run inference — autonomous travel agent system prompt
  pub async fn infer(&self, prompt: &str) -> Result<String> {
    self
      .infer_with_system(
        "You are an autonomous travel planning agent. You plan itineraries, \
       optimise routes under budget constraints, and make intelligent booking \
       decisions. Output structured JSON when asked. Be concise and deterministic.",
        prompt,
        Some(4096),
      )
      .await
  }

  /// Two-turn chain-of-thought (ReAct-style):
  ///   Turn 1 — model reasons freely about the problem (scratchpad)
  ///   Turn 2 — model's own reasoning is fed back as context; it outputs the answer
  ///
  /// This produces much higher quality structured output than a single-turn call
  /// because the model can think before it commits to numbers/names.
  pub async fn think_then_answer(
    &self,
    system: &str,
    think_prompt: &str,
    answer_prompt: &str,
    max_tokens: Option<u32>,
  ) -> Result<String> {
    // Turn 1: free reasoning
    let thinking = self
      .infer_with_system(system, think_prompt, Some(512))
      .await
      .unwrap_or_default();

    // Turn 2: multi-turn — feed reasoning back, then ask for structured output
    let messages = vec![
      ChatMessage {
        role: "system".into(),
        content: system.into(),
      },
      ChatMessage {
        role: "user".into(),
        content: think_prompt.into(),
      },
      ChatMessage {
        role: "assistant".into(),
        content: thinking,
      },
      ChatMessage {
        role: "user".into(),
        content: answer_prompt.into(),
      },
    ];
    self.chat(messages, max_tokens).await
  }

  /// Send a full conversation history to the model and return the next reply.
  async fn chat(&self, messages: Vec<ChatMessage>, max_tokens: Option<u32>) -> Result<String> {
    // Disable chain-of-thought for qwen3.x models to keep latency manageable
    let enable_thinking = if self.model.starts_with("qwen3") {
      Some(false)
    } else {
      None
    };
    let req = ChatCompletionRequest {
      model: self.model.clone(),
      messages,
      max_tokens,
      enable_thinking,
    };

    let mut request = self.http.post(&self.endpoint).json(&req);
    if let Some(key) = &self.api_key {
      request = request.bearer_auth(key);
    }

    let resp = request
      .send()
      .await
      .context("Failed to reach Qwen endpoint")?;

    if !resp.status().is_success() {
      let status = resp.status();
      let body = resp.text().await.unwrap_or_default();
      anyhow::bail!("Qwen returned {}: {}", status, body);
    }

    let parsed: ChatCompletionResponse =
      resp.json().await.context("Failed to parse Qwen response")?;

    parsed
      .choices
      .into_iter()
      .next()
      .map(|c| c.message.content)
      .ok_or_else(|| anyhow::anyhow!("Qwen returned empty choices"))
  }

  /// Run inference with a custom system prompt
  pub async fn infer_with_system(
    &self,
    system: &str,
    user: &str,
    max_tokens: Option<u32>,
  ) -> Result<String> {
    self
      .chat(
        vec![
          ChatMessage {
            role: "system".into(),
            content: system.into(),
          },
          ChatMessage {
            role: "user".into(),
            content: user.into(),
          },
        ],
        max_tokens,
      )
      .await
  }
}

// ─── Builder ──────────────────────────────────────────────────────────────────

/// Build QwenClient from environment variables:
///   QWEN_ENDPOINT  — required
///   QWEN_MODEL     — optional, defaults to qwen-max
///   QWEN_API_KEY   — optional (required for DashScope)
pub fn build_qwen_client() -> Result<QwenClient> {
  let endpoint = std::env::var("QWEN_ENDPOINT").context("QWEN_ENDPOINT not set in .env")?;
  let model = std::env::var("QWEN_MODEL").unwrap_or_else(|_| "qwen-max".to_string());

  Ok(match std::env::var("QWEN_API_KEY") {
    Ok(key) => QwenClient::with_api_key(endpoint, model, key),
    Err(_) => QwenClient::new(endpoint, model),
  })
}
