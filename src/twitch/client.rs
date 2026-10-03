use reqwest::Client;

use crate::{
    core::{
        SecretString,
        consts::{TWITCH_HELIX_USERS_URL, TWITCH_OAUTH_TOKEN_URL, TWITCH_OAUTH_URL},
    },
    twitch::{TwitchAuthentication, TwitchTokenResponse, TwitchUser, TwitchUsersResponse},
    web::AppError,
};

#[derive(Clone)]
pub struct TwitchClient {
    http: Client,
    client_id: SecretString,
    client_secret: SecretString,
    redirect_url: String,
}

impl TwitchClient {
    pub fn new(client_id: SecretString, client_secret: SecretString, redirect_url: String) -> Self {
        Self {
            http: Client::new(),
            client_id,
            client_secret,
            redirect_url,
        }
    }

    pub fn authorization_url(&self, oauth_state: &str) -> String {
        let client_id = self.client_id.clone().unmask();
        let mut url = TWITCH_OAUTH_URL.clone();
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &client_id)
            .append_pair("redirect_uri", &self.redirect_url)
            .append_pair("scope", "chat:read")
            .append_pair("state", oauth_state);
        url.to_string()
    }

    pub async fn exchange_code(&self, code: &str) -> Result<TwitchTokenResponse, AppError> {
        let client_id = self.client_id.clone().unmask();
        let client_secret = self.client_secret.clone().unmask();
        let token = self
            .http
            .post(*TWITCH_OAUTH_TOKEN_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.redirect_url.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<TwitchTokenResponse>()
            .await?;

        Ok(token)
    }

    pub async fn current_user(&self, access_token: &str) -> Result<TwitchUser, AppError> {
        let client_id = self.client_id.clone().unmask();
        let response = self
            .http
            .get(*TWITCH_HELIX_USERS_URL)
            .bearer_auth(access_token)
            .header("Client-Id", &client_id)
            .send()
            .await?
            .error_for_status()?
            .json::<TwitchUsersResponse>()
            .await?;

        response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| AppError::OAuth("twitch returned no user".into()))
    }

    pub async fn authenticate(&self, code: &str) -> Result<TwitchAuthentication, AppError> {
        let token = self.exchange_code(code).await?;

        let user = self.current_user(&token.access_token).await?;

        Ok(TwitchAuthentication { user, token })
    }
}
