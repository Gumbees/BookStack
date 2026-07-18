pub mod auth;
pub mod error;
pub mod markdown;
pub mod models;
pub mod services;
pub mod slug;

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub use error::CoreError;
pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub bind_addr: String,
    pub admin_email: String,
    pub admin_password: String,
    pub static_dir: String,
    /// Public base URL of this instance, used to build clickable links in
    /// MCP responses (e.g. https://docs.example.com).
    pub public_url: String,
    /// IANA timezone name surfaced in MCP `_meta.time` blocks.
    pub timezone: String,
}

impl Config {
    pub fn from_env() -> Config {
        let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            tracing::warn!("JWT_SECRET not set; using a random per-boot secret (sessions reset on restart)");
            use rand::Rng;
            rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(48)
                .map(char::from)
                .collect()
        });
        Config {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://bookstack:bookstack@localhost:5432/bookstack".to_string()),
            jwt_secret,
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            admin_email: std::env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@admin.com".to_string()),
            admin_password: std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "password".to_string()),
            static_dir: std::env::var("STATIC_DIR").unwrap_or_else(|_| "frontend/dist".to_string()),
            public_url: std::env::var("PUBLIC_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string())
                .trim_end_matches('/')
                .to_string(),
            timezone: std::env::var("TIMEZONE").unwrap_or_else(|_| "UTC".to_string()),
        }
    }
}

/// Shared application core: database pool + configuration.
#[derive(Clone)]
pub struct Core {
    pub db: PgPool,
    pub config: Arc<Config>,
}

impl Core {
    /// Connect to Postgres, run migrations, and seed the initial admin user.
    pub async fn connect(config: Config) -> Result<Core> {
        let db = PgPoolOptions::new()
            .max_connections(16)
            .connect(&config.database_url)
            .await?;
        sqlx::migrate!("../../migrations").run(&db).await?;

        if let Some((email, _)) =
            services::users::ensure_admin(&db, &config.admin_email, &config.admin_password).await?
        {
            tracing::info!("seeded initial admin user: {email}");
        }

        Ok(Core { db, config: Arc::new(config) })
    }
}
