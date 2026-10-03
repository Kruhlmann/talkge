use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::{twitch::TwitchChatMessage, web::AppError};

const TWITCH_IRC_URL: &str = "wss://irc-ws.chat.twitch.tv:443";

pub struct TwitchChatClient {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    login: String,
}

impl TwitchChatClient {
    pub async fn connect(login: impl Into<String>, access_token: &str) -> Result<Self, AppError> {
        let login = login.into();
        let formalities = [
            format!("PASS oauth:{access_token}\r\n").into(),
            format!("NICK {login}\r\n").into(),
            "CAP REQ :twitch.tv/tags twitch.tv/commands\r\n".into(),
            format!("JOIN #{login}\r\n").into(),
        ];
        let (mut socket, _) = connect_async(TWITCH_IRC_URL).await?;
        for formality in formalities {
            socket
                .send(Message::Text(formality))
                .await
                .inspect_err(|error| tracing::error!(?error, "send irc"))?;
        }
        tracing::info!(
            %login,
            "connected to twitch chat"
        );

        Ok(Self { socket, login })
    }

    pub async fn next_message(&mut self) -> Result<Option<TwitchChatMessage>, AppError> {
        while let Some(message) = self.socket.next().await {
            match message? {
                Message::Text(text) => {
                    let text = text.to_string();
                    if text.starts_with("PING ") {
                        let pong = text.replacen("PING", "PONG", 1);
                        self.socket.send(Message::Text(pong.into())).await?;
                        continue;
                    }
                    for line in text.lines() {
                        if let Some(message) = TwitchChatMessage::parse(line) {
                            return Ok(Some(message));
                        } else {
                            tracing::debug!(?line, "parsing twitch chat message failed")
                        }
                    }
                }

                Message::Close(_) => {
                    return Ok(None);
                }

                _ => {}
            }
        }

        Ok(None)
    }
}
