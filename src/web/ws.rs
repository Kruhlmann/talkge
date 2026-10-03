use std::{collections::VecDeque, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    core::consts::MAX_VOICE_QUEUE,
    llm::{VoiceClient, VoiceOutput, VoiceRequest},
    twitch::{TwitchChatClient, TwitchCredentialsStore},
    web::AppError,
};

pub struct CanvasWebSocketClient {
    socket: WebSocket,
    user_id: String,
    twitch_credentials: TwitchCredentialsStore,
    voice_queue: VecDeque<VoiceRequest>,
    active_voice: Option<VoiceRequest>,
    voice_client: Arc<VoiceClient>,
    active_voice_task: Option<JoinHandle<Result<(), AppError>>>,
}

impl CanvasWebSocketClient {
    pub fn new(
        socket: WebSocket,
        user_id: String,
        twitch_credentials: TwitchCredentialsStore,
        voice_client: Arc<VoiceClient>,
    ) -> Self {
        Self {
            socket,
            user_id,
            twitch_credentials,
            voice_queue: VecDeque::with_capacity(*MAX_VOICE_QUEUE),
            active_voice: None,
            voice_client,
            active_voice_task: None,
        }
    }

    pub async fn run(mut self) -> Result<(), AppError> {
        tracing::info!(user = self.user_id, "canvas websocket connected");
        let (voice_tx, mut voice_rx) = mpsc::channel::<VoiceOutput>(16);
        let credentials = self
            .twitch_credentials
            .get(&self.user_id)
            .await?
            .ok_or_else(|| AppError::Other("no Twitch credentials for user".into()))?;
        let mut twitch =
            TwitchChatClient::connect(&credentials.login, &credentials.access_token).await?;

        loop {
            tokio::select! {
                canvas_message = self.socket.recv() => {
                    match canvas_message {
                        Some(Ok(Message::Ping(data))) => {
                            self.socket
                                .send(Message::Pong(data))
                                .await
                                .map_err(|error| {
                                    AppError::Other(
                                        error.to_string()
                                    )
                                })?;
                        }
                        Some(Ok(Message::Close(frame))) => {
                            tracing::info!(
                                user = self.user_id,
                                ?frame,
                                "canvas websocket closed"
                            );

                            break;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(error)) => {
                            tracing::debug!(
                                user = self.user_id,
                                ?error,
                                "canvas websocket error"
                            );

                            break;
                        }
                        None => { break; }
                    }
                }

                output = voice_rx.recv() => {
                    match output {
                        Some(VoiceOutput::Started) => {
                            self.socket
                                .send(Message::Text(
                                    r#"{"type":"voice_start"}"#
                                        .into()
                                ))
                                .await?;
                        }

                        Some(VoiceOutput::Audio(bytes)) => {
                            self.socket
                                .send(Message::Binary(bytes))
                                .await?;
                        }

                        Some(VoiceOutput::Finished) => {
                            self.socket
                                .send(Message::Text(
                                    r#"{"type":"voice_end"}"#
                                        .into()
                                ))
                                .await?;

                            self.finish_voice_request(
                                voice_tx.clone()
                            )
                            .await;
                        }

                        None => {
                            tracing::debug!(
                                "voice output channel closed"
                            );

                            break;
                        }
                    }
                }

                twitch_message = twitch.next_message() => {
                    match twitch_message? {
                        Some(message) => {
                            if let Some(request) =
                                VoiceRequest::from_chat(
                                    message.username,
                                    &message.message,
                                )
                            {
                                self.enqueue_voice_request(
                                    request,
                                    voice_tx.clone(),
                                );
                            }
                        }

                        None => {
                            tracing::warn!(
                                user = self.user_id,
                                "twitch chat disconnected"
                            );

                            break;
                        }
                    }
                }
            }
        }

        // Explicitly cancel generation when the canvas
        // disconnects. Dropping a JoinHandle alone does
        // not cancel its task.
        if let Some(task) = self.active_voice_task.take() {
            task.abort();
        }

        self.active_voice = None;
        self.voice_queue.clear();

        tracing::info!(user = self.user_id, "canvas websocket disconnected");

        Ok(())
    }

    fn start_voice_request(&mut self, request: VoiceRequest, tx: mpsc::Sender<VoiceOutput>) {
        let voice = Arc::clone(&self.voice_client);

        let prompt = format!(
            "Username: {}\nMessage: {}",
            request.username, request.message,
        );

        tracing::info!(
            username = request.username,
            message = request.message,
            "starting voice request"
        );

        self.active_voice = Some(request);

        self.active_voice_task = Some(tokio::spawn(async move {
            if tx.send(VoiceOutput::Started).await.is_err() {
                return Ok(());
            }

            let result = voice.generate(&prompt, tx.clone()).await;

            let _ = tx.send(VoiceOutput::Finished).await;

            result
        }));
    }

    async fn finish_voice_request(&mut self, tx: mpsc::Sender<VoiceOutput>) {
        if let Some(task) = self.active_voice_task.take() {
            match task.await {
                Ok(Ok(())) => {}

                Ok(Err(error)) => {
                    tracing::error!(?error, "voice generation failed");
                }

                Err(error) => {
                    tracing::error!(?error, "voice generation task failed");
                }
            }
        }

        self.active_voice = None;
        if let Some(request) = self.voice_queue.pop_front() {
            self.start_voice_request(request, tx);
        }
    }

    fn enqueue_voice_request(&mut self, request: VoiceRequest, tx: mpsc::Sender<VoiceOutput>) {
        if self.active_voice_task.is_none() {
            self.start_voice_request(request, tx);
            return;
        }
        if self.voice_queue.len() < *MAX_VOICE_QUEUE {
            tracing::info!(
                username = request.username,
                queue_len = self.voice_queue.len() + 1,
                "queued voice request"
            );

            self.voice_queue.push_back(request);
        } else {
            tracing::warn!(
                username = request.username,
                "voice queue full, dropping request"
            );
        }
    }
}
