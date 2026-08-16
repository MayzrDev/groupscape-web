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
#[derive(Deserialize, Clone)]
pub struct Config {
    pub pg: deadpool_postgres::Config,
    #[serde(default = "default_logger_config")]
    pub logger: LoggerConfig,
    #[serde(default = "default_captcha_config")]
    pub hcaptcha: CaptchaConfig,
    #[serde(default = "default_admin_config")]
    pub admin: AdminConfig,
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
impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string("config.toml")?;
        Ok(basic_toml::from_str(&config_str)?)
    }
}
