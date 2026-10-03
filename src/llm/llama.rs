use std::pin::Pin;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::web::AppError;

#[derive(Clone)]
pub struct LlamaClient {
    http: reqwest::Client,
    base_url: String,
    model: String,
}

impl LlamaClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    pub async fn stream_chat(
        &self,
        messages: Vec<LlamaMessage>,
    ) -> Result<LlamaTextStream, AppError> {
        tracing::info!(?messages, "streaming");
        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .http
            .post(url)
            .json(&LlamaChatRequest {
                model: &self.model,
                messages: &messages,
                stream: true,
            })
            .send()
            .await?
            .error_for_status()?;

        Ok(LlamaTextStream::new(response.bytes_stream()))
    }

    pub async fn stream_prompt(
        &self,
        prompt: impl Into<String>,
    ) -> Result<LlamaTextStream, AppError> {
        self.stream_chat(vec![LlamaMessage::user(prompt)]).await
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LlamaMessage {
    pub role: LlamaRole,
    pub content: String,
}

impl LlamaMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: LlamaRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: LlamaRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: LlamaRole::Assistant,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LlamaRole {
    System,
    User,
    Assistant,
}

#[derive(Serialize)]
struct LlamaChatRequest<'a> {
    model: &'a str,
    messages: &'a [LlamaMessage],
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct LlamaChatChunk {
    choices: Vec<LlamaChoice>,
}

#[derive(Debug, Deserialize)]
struct LlamaChoice {
    delta: LlamaDelta,
}

#[derive(Debug, Deserialize)]
struct LlamaDelta {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

pub struct LlamaTextStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    buffer: String,
    finished: bool,
}

impl LlamaTextStream {
    fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
    {
        Self {
            inner: Box::pin(stream),
            buffer: String::new(),
            finished: false,
        }
    }

    pub async fn next_text(&mut self) -> Result<Option<String>, AppError> {
        loop {
            if self.finished {
                return Ok(None);
            }
            if let Some(event) = self.take_event()? {
                if event == "[DONE]" {
                    self.finished = true;
                    return Ok(None);
                }
                let chunk: LlamaChatChunk = serde_json::from_str(&event)?;
                for choice in chunk.choices {
                    if let Some(reasoning) = choice.delta.reasoning_content {
                        tracing::debug!(
                            %reasoning,
                            "llama reasoning"
                        );
                    }
                    if let Some(content) = choice.delta.content
                        && !content.is_empty() {
                            return Ok(Some(content));
                        }
                }
                continue;
            }

            match self.inner.next().await {
                Some(Ok(bytes)) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&bytes));
                }
                Some(Err(error)) => {
                    return Err(error.into());
                }
                None => {
                    self.finished = true;
                    return Ok(None);
                }
            }
        }
    }

    fn take_event(&mut self) -> Result<Option<String>, AppError> {
        let (position, delimiter_len) = if let Some(position) = self.buffer.find("\r\n\r\n") {
            (position, 4)
        } else if let Some(position) = self.buffer.find("\n\n") {
            (position, 2)
        } else {
            return Ok(None);
        };

        let event = self.buffer[..position].to_string();

        self.buffer.drain(..position + delimiter_len);

        let data = event
            .lines()
            .filter_map(|line| {
                line.trim_end_matches('\r')
                    .strip_prefix("data:")
                    .map(str::trim)
            })
            .collect::<Vec<_>>()
            .join("\n");

        if data.is_empty() {
            return Ok(None);
        }

        Ok(Some(data))
    }
}
