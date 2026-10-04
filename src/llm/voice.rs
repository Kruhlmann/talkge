use bytes::Bytes;

use crate::{
    llm::{LlamaClient, TtsClient},
    web::AppError,
};

#[async_trait::async_trait]
pub trait VoiceClient: Send + Sync {
    async fn generate(
        &self,
        prompt: &str,
        tx: tokio::sync::mpsc::Sender<VoiceOutput>,
    ) -> Result<(), AppError>;
    fn should_speak(text: &str) -> bool;
}

#[derive(Clone)]
pub struct VoiceHttpClient {
    llama: LlamaClient,
    tts: TtsClient,
}

#[derive(Debug)]
pub enum VoiceOutput {
    Started,
    Audio(Bytes),
    Finished,
}

impl VoiceHttpClient {
    pub fn new(llama: LlamaClient, tts: TtsClient) -> Self {
        Self { llama, tts }
    }
}

#[async_trait::async_trait]
impl VoiceClient for VoiceHttpClient {
    async fn generate(
        &self,
        prompt: &str,
        tx: tokio::sync::mpsc::Sender<VoiceOutput>,
    ) -> Result<(), AppError> {
        let mut stream = self.llama.stream_prompt(prompt).await?;
        let mut speech_buffer = String::new();

        while let Some(text) = stream.next_text().await? {
            speech_buffer.push_str(&text);
            if Self::should_speak(&speech_buffer) {
                let text = std::mem::take(&mut speech_buffer);
                let audio = self.tts.speech(&text).await?;
                if tx.send(VoiceOutput::Audio(audio)).await.is_err() {
                    return Ok(());
                }
            }
        }

        if !speech_buffer.trim().is_empty() {
            let audio = self.tts.speech(&speech_buffer).await?;
            if tx.send(VoiceOutput::Audio(audio)).await.is_err() {
                return Ok(());
            }
        }

        Ok(())
    }

    fn should_speak(text: &str) -> bool {
        let text = text.trim_end();
        text.ends_with('.') || text.ends_with('!') || text.ends_with('?') || text.len() >= 180
    }
}

#[derive(Debug, Clone)]
pub struct VoiceRequest {
    pub username: String,
    pub message: String,
}

impl VoiceRequest {
    pub fn from_chat(username: impl Into<String>, message: &str) -> Option<Self> {
        const PREFIX: &str = "@talkge";
        if message.len() < PREFIX.len() {
            return None;
        }

        let (prefix, rest) = message.split_at(PREFIX.len());
        if !prefix.eq_ignore_ascii_case(PREFIX) {
            return None;
        }

        let message = rest.trim();
        if message.is_empty() {
            return None;
        }

        Some(Self {
            username: username.into(),
            message: message.to_string(),
        })
    }
}
