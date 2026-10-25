use std::env;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub cookie_secure: bool,
    pub traq: TraqSettings,
    pub notes: NotesBackend,
}

#[derive(Clone, Debug)]
pub struct TraqSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub auth_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub scope: String,
}

#[derive(Clone, Debug)]
pub enum NotesBackend {
    Memory,
    Dynamo {
        region: String,
        table: String,
        endpoint: Option<String>,
    },
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let port = env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .context("PORT must be a number")?;
        let cookie_secure = matches!(
            env::var("COOKIE_SECURE").ok().as_deref(),
            Some("1") | Some("true")
        );
        let notes = match env::var("NOTES_BACKEND").ok().as_deref() {
            Some("dynamodb") => NotesBackend::Dynamo {
                region: env::var("AWS_REGION").unwrap_or_else(|_| "ap-northeast-1".to_string()),
                table: env::var("NOTES_TABLE").unwrap_or_else(|_| "yarimashow-notes".to_string()),
                endpoint: env::var("AWS_ENDPOINT_URL").ok(),
            },
            _ => NotesBackend::Memory,
        };

        Ok(Self {
            port,
            cookie_secure,
            traq: TraqSettings {
                client_id: env::var("CLIENT_ID").context("Missing CLIENT_ID")?,
                client_secret: env::var("CLIENT_SECRET").context("Missing CLIENT_SECRET")?,
                redirect_url: env::var("REDIRECT_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/authorized".to_string()),
                auth_url: env::var("AUTH_URL")
                    .unwrap_or_else(|_| "https://q.trap.jp/api/v3/oauth2/authorize".to_string()),
                token_url: env::var("TOKEN_URL")
                    .unwrap_or_else(|_| "https://q.trap.jp/api/v3/oauth2/token".to_string()),
                userinfo_url: env::var("USERINFO_URL")
                    .unwrap_or_else(|_| "https://q.trap.jp/api/v3/users/me".to_string()),
                scope: env::var("TRAQ_SCOPE").unwrap_or_else(|_| "read".to_string()),
            },
            notes,
        })
    }
}
