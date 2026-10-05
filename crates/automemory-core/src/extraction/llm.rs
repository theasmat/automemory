use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::db::Database;
use crate::models::{
    Memory, MemoryScope, MemorySourceKind, MemoryStatus, MemoryType, NormalizedEvent,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmProviderConfig {
    Ollama {
        endpoint: String,
        model: String,
    },
    OpenAi {
        endpoint: String,
        api_key: String,
        model: String,
    },
    Anthropic {
        api_key: String,
        model: String,
    },
}

impl Default for LlmProviderConfig {
    fn default() -> Self {
        Self::Ollama {
            endpoint: "http://localhost:11434".to_string(),
            model: "llama3.2".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedItem {
    pub r#type: String,
    pub summary: String,
    pub content: String,
    pub tags: Vec<String>,
    pub confidence: f64,
}

pub struct LlmExtractor {
    db: Arc<Database>,
    client: reqwest::Client,
}

impl LlmExtractor {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Run LLM-assisted extraction over a sequence of events.
    pub async fn extract_from_events(
        &self,
        config: &LlmProviderConfig,
        project_id: Option<&str>,
        events: &[NormalizedEvent],
    ) -> Result<Vec<Memory>> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        let mut events_summary = String::new();
        for ev in events {
            if let Some(tool) = &ev.tool {
                let status_icon = if tool.status == "success" {
                    "✓"
                } else {
                    "✗"
                };
                events_summary.push_str(&format!(
                    "- [{}] {} ({})",
                    status_icon, tool.name, tool.category
                ));
                if let Some(inp) = &tool.input {
                    let s = inp.to_string();
                    let truncated = if s.len() > 200 { &s[..200] } else { &s };
                    events_summary.push_str(&format!(" input: {}", truncated));
                }
                events_summary.push('\n');
            }
        }

        let system_prompt = r#"You are AutoMemory's extraction engine. Analyze the following tool events from an AI coding session.
Extract only durable facts, architectural decisions, bug root-causes and fixes, or project conventions.
Ignore trivial actions, generic greetings, and standard directory listings.

Respond ONLY with a JSON array of extracted items in this format:
[
  {
    "type": "decision" | "lesson" | "pattern" | "fact",
    "summary": "Concise 1-line summary",
    "content": "Detailed explanation with context and rationale",
    "tags": ["tag1", "tag2"],
    "confidence": 0.85
  }
]
If nothing durable was learned, respond with []"#;

        let user_prompt = format!("Session Events:\n{}", events_summary);

        let response_text = match config {
            LlmProviderConfig::Ollama { endpoint, model } => {
                let url = format!("{}/api/generate", endpoint.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "prompt": format!("{}\n\n{}", system_prompt, user_prompt),
                    "stream": false,
                    "format": "json"
                });

                let res = self.client.post(&url).json(&body).send().await?;
                let json_res: serde_json::Value = res.json().await?;
                json_res["response"].as_str().unwrap_or("[]").to_string()
            }
            LlmProviderConfig::OpenAi {
                endpoint,
                api_key,
                model,
            } => {
                let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "temperature": 0.2
                });

                let res = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&body)
                    .send()
                    .await?;
                let json_res: serde_json::Value = res.json().await?;
                json_res["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("[]")
                    .to_string()
            }
            LlmProviderConfig::Anthropic { api_key, model } => {
                let url = "https://api.anthropic.com/v1/messages";
                let body = serde_json::json!({
                    "model": model,
                    "max_tokens": 2048,
                    "system": system_prompt,
                    "messages": [
                        {"role": "user", "content": user_prompt}
                    ]
                });

                let res = self
                    .client
                    .post(url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&body)
                    .send()
                    .await?;
                let json_res: serde_json::Value = res.json().await?;
                json_res["content"][0]["text"]
                    .as_str()
                    .unwrap_or("[]")
                    .to_string()
            }
        };

        // Parse extracted items
        let items: Vec<ExtractedItem> = serde_json::from_str(&response_text)
            .or_else(|_| {
                // If model wrapped in markdown fences
                if let Some(start) = response_text.find('[') {
                    if let Some(end) = response_text.rfind(']') {
                        return serde_json::from_str(&response_text[start..=end]);
                    }
                }
                Ok(Vec::new())
            })
            .unwrap_or_default();

        let mut saved_memories = Vec::new();
        for item in items {
            let mem_type = match item.r#type.to_lowercase().as_str() {
                "decision" => MemoryType::Decision,
                "lesson" => MemoryType::Lesson,
                "pattern" => MemoryType::Pattern,
                "warning" => MemoryType::Warning,
                _ => MemoryType::Fact,
            };

            let mem = Memory {
                id: Uuid::now_v7().to_string(),
                scope: if project_id.is_some() {
                    MemoryScope::Project
                } else {
                    MemoryScope::Global
                },
                project_id: project_id.map(String::from),
                memory_type: mem_type,
                content: item.content,
                summary: item.summary,
                confidence: item.confidence.clamp(0.0, 1.0),
                importance: 0.7,
                freshness: 1.0,
                status: MemoryStatus::Active,
                source_kind: MemorySourceKind::TaskReflection,
                version: Some(1),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_recalled_at: None,
                tags: item.tags,
                entities: Vec::new(),
                metadata: serde_json::json!({"extracted_by": "llm"}),
            };

            if let Ok(()) = self.db.save_memory(&mem) {
                info!("LLM saved memory: {}", mem.summary);
                saved_memories.push(mem);
            }
        }

        Ok(saved_memories)
    }

    /// Check whether two memory statements contradict each other using the configured LLM.
    /// Returns Some(explanation) if contradiction detected, None otherwise.
    pub async fn check_contradiction(
        &self,
        config: &LlmProviderConfig,
        stmt_a: &str,
        stmt_b: &str,
    ) -> Result<Option<String>> {
        let system_prompt = r#"You are an architecture consistency validator for AutoMemory.
Analyze the two statements below. Determine if Statement A and Statement B contradict each other (for example: opposing architectural decisions, incompatible framework/database choices, or direct policy conflicts).
Respond ONLY in JSON with:
{
  "contradicts": true,
  "explanation": "Brief explanation of the contradiction"
}
or
{
  "contradicts": false,
  "explanation": ""
}"#;

        let user_prompt = format!("Statement A: {}\n\nStatement B: {}", stmt_a, stmt_b);

        let response_text = match config {
            LlmProviderConfig::Ollama { endpoint, model } => {
                let url = format!("{}/api/generate", endpoint.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "prompt": format!("{}\n\n{}", system_prompt, user_prompt),
                    "stream": false,
                    "format": "json"
                });
                let res = self.client.post(&url).json(&body).send().await?;
                let json_res: serde_json::Value = res.json().await?;
                json_res["response"].as_str().unwrap_or("{}").to_string()
            }
            LlmProviderConfig::OpenAi {
                endpoint,
                api_key,
                model,
            } => {
                let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
                let body = serde_json::json!({
                    "model": model,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "temperature": 0.0
                });
                let res = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&body)
                    .send()
                    .await?;
                let json_res: serde_json::Value = res.json().await?;
                json_res["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("{}")
                    .to_string()
            }
            LlmProviderConfig::Anthropic { api_key, model } => {
                let url = "https://api.anthropic.com/v1/messages";
                let body = serde_json::json!({
                    "model": model,
                    "max_tokens": 1024,
                    "system": system_prompt,
                    "messages": [{"role": "user", "content": user_prompt}]
                });
                let res = self
                    .client
                    .post(url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&body)
                    .send()
                    .await?;
                let json_res: serde_json::Value = res.json().await?;
                json_res["content"][0]["text"]
                    .as_str()
                    .unwrap_or("{}")
                    .to_string()
            }
        };

        #[derive(Deserialize)]
        struct ContradictionRes {
            contradicts: bool,
            explanation: Option<String>,
        }

        let parsed: Option<ContradictionRes> = serde_json::from_str(&response_text)
            .or_else(|_| {
                if let Some(s) = response_text.find('{') {
                    if let Some(e) = response_text.rfind('}') {
                        return serde_json::from_str(&response_text[s..=e]);
                    }
                }
                Ok(ContradictionRes {
                    contradicts: false,
                    explanation: None,
                })
            })
            .ok();

        if let Some(res) = parsed {
            if res.contradicts {
                return Ok(Some(res.explanation.unwrap_or_else(|| {
                    "Contradictory architectural statements".to_string()
                })));
            }
        }
        Ok(None)
    }
}
