use bytes::Bytes;
use serde::Serialize;

use crate::web::AppError;

#[derive(Clone)]
pub struct TtsClient {
    http: reqwest::Client,
    base_url: String,
    voice: String,
}

impl TtsClient {
    pub fn new(base_url: impl Into<String>, voice: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
            voice: voice.into(),
        }
    }

    pub async fn speech(&self, text: &str) -> Result<Bytes, AppError> {
        tracing::info!(?text, "generate speech");
        let response = self
            .http
            .post(format!("{}/v1/audio/speech", self.base_url))
            .json(&SpeechRequest {
                model: "kokoro",
                voice: &self.voice,
                input: text,
                response_format: "mp3",
                stream: false,
            })
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await?;

            tracing::error!(
                %status,
                %body,
                "tts request failed"
            );

            return Err(AppError::Other(format!("TTS returned {status}: {body}")));
        }

        Ok(response.bytes().await?)
    }
}

#[derive(Serialize)]
pub struct SpeechRequest<'a> {
    pub model: &'a str,
    pub voice: &'a str,
    pub input: &'a str,
    pub response_format: &'a str,
    pub stream: bool,
}
