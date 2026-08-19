use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}
impl LogLevel {
    pub fn to_string(&self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}
#[derive(Deserialize, Clone)]
pub struct LoggerConfig {
    pub level: LogLevel,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct CaptchaConfig {
    pub enabled: bool,
    pub sitekey: String,
    #[serde(skip_serializing)]
    pub secret: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct AdminConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token_hash: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct DiscordConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub client_id: String,
    #[serde(default, skip_serializing)]
    pub client_secret: String,
    #[serde(default)]
    pub redirect_uri: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct PushConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub vapid_public_key: String,
    #[serde(default, skip_serializing)]
    pub vapid_private_key: String,
    #[serde(default)]
    pub vapid_subject: String,
}
#[derive(Deserialize, Clone)]
pub struct Config {
    pub pg: deadpool_postgres::Config,
    #[serde(default = "default_logger_config")]
    pub logger: LoggerConfig,
    #[serde(default = "default_captcha_config")]
    pub hcaptcha: CaptchaConfig,
    #[serde(default = "default_admin_config")]
    pub admin: AdminConfig,
    #[serde(default = "default_discord_config")]
    pub discord: DiscordConfig,
    #[serde(default = "default_push_config")]
    pub push: PushConfig,
    /// Frontend origin to send the browser back to once a Discord OAuth login completes.
    /// Only ever set from the `WEB_ORIGIN` env var (see `main.rs`), same as `admin.token_hash`
    /// is only ever set from `ADMIN_TOKEN` - never written to `config.toml`.
    #[serde(default)]
    pub web_origin: String,
}
fn default_logger_config() -> LoggerConfig {
    LoggerConfig {
        level: LogLevel::Info,
    }
}
fn default_captcha_config() -> CaptchaConfig {
    CaptchaConfig {
        enabled: false,
        sitekey: "".to_string(),
        secret: "".to_string(),
    }
}
fn default_admin_config() -> AdminConfig {
    AdminConfig {
        enabled: false,
        token_hash: "".to_string(),
    }
}
fn default_discord_config() -> DiscordConfig {
    DiscordConfig {
        enabled: false,
        client_id: "".to_string(),
        client_secret: "".to_string(),
        redirect_uri: "".to_string(),
    }
}
fn default_push_config() -> PushConfig {
    PushConfig {
        enabled: false,
        vapid_public_key: "".to_string(),
        vapid_private_key: "".to_string(),
        vapid_subject: "".to_string(),
    }
}
impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string("config.toml")?;
        Ok(basic_toml::from_str(&config_str)?)
    }
}
