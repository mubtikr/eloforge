use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

impl ChatRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub session_id: Uuid,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: u32,
    pub purpose: String,
}

impl LlmRequest {
    pub fn new(session_id: Uuid, purpose: impl Into<String>) -> Self {
        Self {
            session_id,
            messages: Vec::new(),
            temperature: Some(0.3),
            max_tokens: 900,
            purpose: purpose.into(),
        }
    }

    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage {
            role: ChatRole::System,
            content: content.into(),
        });
        self
    }

    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage {
            role: ChatRole::User,
            content: content.into(),
        });
        self
    }

    pub fn max_tokens(mut self, value: u32) -> Self {
        self.max_tokens = value;
        self
    }
}

#[derive(Debug, Clone)]
pub enum LlmEvent {
    Delta(String),
    Done { tokens: u64 },
}

pub type LlmStream = Pin<Box<dyn Stream<Item = AppResult<LlmEvent>> + Send>>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn model(&self) -> &str;
    fn is_configured(&self) -> bool;
    async fn stream(&self, request: LlmRequest) -> AppResult<LlmStream>;
}

pub async fn collect_text(mut stream: LlmStream) -> AppResult<(String, u64)> {
    use futures::StreamExt;

    let mut text = String::new();
    let mut tokens = 0u64;
    while let Some(event) = stream.next().await {
        match event? {
            LlmEvent::Delta(delta) => text.push_str(&delta),
            LlmEvent::Done { tokens: reported } => {
                tokens = reported;
                break;
            }
        }
    }
    Ok((text, tokens))
}
