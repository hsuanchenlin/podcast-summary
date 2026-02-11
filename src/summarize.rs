use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
}

pub struct SummaryResult {
    pub content: String,
    pub model: String,
    pub prompt_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a podcast summarizer. Given a transcript of a podcast episode, produce a structured summary with the following sections:

TOPICS: List the main topics discussed (comma-separated)

SUMMARY: A concise narrative summary (2-3 paragraphs)

KEY TAKEAWAYS:
- Bullet points of the most important insights and conclusions

NOTABLE QUOTES:
- Direct quotes with approximate timestamps if available

Be concise but comprehensive. Focus on actionable insights and key information."#;

pub async fn generate_summary(
    client: &reqwest::Client,
    api_base_url: &str,
    api_key: &str,
    model: &str,
    max_tokens: u32,
    system_prompt: Option<&str>,
    transcript: &str,
) -> Result<SummaryResult> {
    let system = system_prompt.unwrap_or(DEFAULT_SYSTEM_PROMPT);

    let request = ChatRequest {
        model: model.to_string(),
        max_tokens,
        messages: vec![
            Message {
                role: "system".to_string(),
                content: system.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!("Here is the podcast transcript to summarize:\n\n{transcript}"),
            },
        ],
    };

    let url = format!("{}/chat/completions", api_base_url.trim_end_matches('/'));

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .with_context(|| format!("Failed to call LLM API at {url}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(crate::error::AppError::ClaudeApi {
            status: status.as_u16(),
            body,
        }
        .into());
    }

    let chat_resp: ChatResponse = response
        .json()
        .await
        .context("Failed to parse LLM API response")?;

    let content = chat_resp
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("")
        .to_string();

    Ok(SummaryResult {
        content,
        model: model.to_string(),
        prompt_tokens: chat_resp.usage.as_ref().and_then(|u| u.prompt_tokens),
        output_tokens: chat_resp.usage.as_ref().and_then(|u| u.completion_tokens),
    })
}
