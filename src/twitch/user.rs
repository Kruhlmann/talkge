use serde::Deserialize;

#[derive(Deserialize)]
pub struct TwitchUsersResponse {
    pub data: Vec<TwitchUser>,
}

#[derive(Deserialize)]
pub struct TwitchUser {
    pub id: String,
    pub login: String,
    pub display_name: String,
}
