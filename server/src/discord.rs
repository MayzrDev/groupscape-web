use crate::config::DiscordConfig;
use crate::error::ApiError;
use serde::Deserialize;
use tokio::task;

const AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
const TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const USER_URL: &str = "https://discord.com/api/users/@me";

pub struct DiscordUser {
    pub id: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
}

#[derive(Deserialize)]
struct UserResponse {
    id: Option<String>,
}

/// `identify` scope only - just enough for a stable Discord user id, matching
/// `groupscape-old`'s `discordOAuthClient.ts` decision (ported here, not the raw code, since
/// this stack uses blocking `ureq` rather than `fetch`).
pub fn authorize_url(config: &DiscordConfig, state: &str) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope=identify&state={}",
        AUTHORIZE_URL,
        urlencoding::encode(&config.client_id),
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(state),
    )
}

pub async fn exchange_code(config: &DiscordConfig, code: &str) -> Result<DiscordUser, ApiError> {
    let client_id = config.client_id.clone();
    let client_secret = config.client_secret.clone();
    let redirect_uri = config.redirect_uri.clone();
    let code = code.to_owned();

    task::spawn_blocking(move || {
        let token_res = ureq::post(TOKEN_URL)
            .send_form([
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
            ])
            .map_err(ApiError::UreqError)?
            .body_mut()
            .read_json::<TokenResponse>()
            .map_err(ApiError::UreqError)?;

        let Some(access_token) = token_res.access_token else {
            return Err(ApiError::DiscordOAuthError(
                "token exchange returned no access_token".to_string(),
            ));
        };

        let user_res = ureq::get(USER_URL)
            .header("Authorization", &format!("Bearer {}", access_token))
            .call()
            .map_err(ApiError::UreqError)?
            .body_mut()
            .read_json::<UserResponse>()
            .map_err(ApiError::UreqError)?;

        match user_res.id {
            Some(id) => Ok(DiscordUser { id }),
            None => Err(ApiError::DiscordOAuthError(
                "user fetch returned incomplete data".to_string(),
            )),
        }
    })
    .await
    .unwrap()
}
