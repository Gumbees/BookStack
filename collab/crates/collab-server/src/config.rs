use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub jwt_secret: String,
    pub max_connections: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env::var("COLLAB_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7700),
            jwt_secret: env::var("COLLAB_JWT_SECRET").unwrap_or_default(),
            max_connections: env::var("COLLAB_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
        }
    }
}
