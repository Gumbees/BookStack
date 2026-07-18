//! External OIDC/OAuth2 login ("Sign in with …").
//!
//! `GET /api/auth/oidc/{id}/start` redirects to the provider; the provider
//! sends the browser back to `GET /api/auth/oidc/callback`, which exchanges
//! the code, fetches userinfo, finds-or-creates the user, and lands on the
//! SPA with `#sso_token=<jwt>`.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use bookstack_core::models::Role;
use bookstack_core::services::{auth_providers, orgs, users};
use bookstack_core::{auth, CoreError};

use crate::error::ApiResult;
use crate::state::AppState;

/// Providers to render on a login surface. Optional `org_id` narrows to an
/// org's effective set (global-inherited + own).
#[derive(Deserialize)]
pub struct ProvidersQuery {
    pub org_id: Option<i64>,
}

pub async fn public_providers(
    State(state): State<AppState>,
    Query(query): Query<ProvidersQuery>,
) -> ApiResult<Json<Value>> {
    let providers = auth_providers::effective_public(&state.core.db, query.org_id).await?;
    Ok(Json(json!({ "data": providers })))
}

/// Signed state passed through the IdP round-trip.
#[derive(Serialize, Deserialize)]
struct SsoState {
    provider_id: i64,
    redirect: String,
    exp: i64,
}

fn sign_state(secret: &str, provider_id: i64, redirect: &str) -> Result<String, CoreError> {
    let claims = SsoState {
        provider_id,
        redirect: redirect.to_string(),
        exp: (chrono::Utc::now() + chrono::Duration::minutes(10)).timestamp(),
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(CoreError::internal)
}

fn verify_state(secret: &str, token: &str) -> Result<SsoState, CoreError> {
    decode::<SsoState>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| CoreError::Unauthorized)
}

#[derive(Deserialize)]
pub struct StartQuery {
    pub redirect: Option<String>,
}

pub async fn start(
    State(state): State<AppState>,
    Path(provider_id): Path<i64>,
    Query(query): Query<StartQuery>,
) -> ApiResult<Response> {
    let provider = auth_providers::get(&state.core.db, provider_id).await?;
    if !provider.enabled {
        return Err(CoreError::NotFound.into());
    }
    // Only same-app relative paths may be used as the post-login redirect.
    let redirect = query
        .redirect
        .filter(|r| r.starts_with('/') && !r.starts_with("//"))
        .unwrap_or_else(|| "/".to_string());
    let sso_state = sign_state(&state.core.config.jwt_secret, provider_id, &redirect)?;
    let callback = format!("{}/api/auth/oidc/callback", state.core.config.public_url);

    let url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
        provider.authorize_url,
        urlencode(&provider.client_id),
        urlencode(&callback),
        urlencode(&provider.scopes),
        urlencode(&sso_state),
    );
    Ok(Redirect::temporary(&url).into_response())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    match run_callback(&state, query).await {
        Ok(redirect) => Redirect::temporary(&redirect).into_response(),
        Err(message) => {
            let target = format!(
                "{}/login#sso_error={}",
                state.core.config.public_url,
                urlencode(&message)
            );
            Redirect::temporary(&target).into_response()
        }
    }
}

async fn run_callback(state: &AppState, query: CallbackQuery) -> Result<String, String> {
    if let Some(error) = query.error {
        return Err(format!("provider returned: {error}"));
    }
    let code = query.code.ok_or("missing code")?;
    let sso_state = verify_state(
        &state.core.config.jwt_secret,
        query.state.as_deref().unwrap_or(""),
    )
    .map_err(|_| "invalid or expired state".to_string())?;
    let provider = auth_providers::get(&state.core.db, sso_state.provider_id)
        .await
        .map_err(|_| "unknown provider".to_string())?;

    // Exchange the code.
    let callback = format!("{}/api/auth/oidc/callback", state.core.config.public_url);
    let token_response: Value = state
        .http
        .post(&provider.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", callback.as_str()),
            ("client_id", provider.client_id.as_str()),
            ("client_secret", provider.client_secret.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("token exchange failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("token exchange returned invalid JSON: {e}"))?;
    let access_token = token_response
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("token response missing access_token")?;

    // Identity from the userinfo endpoint.
    let userinfo: Value = state
        .http
        .get(&provider.userinfo_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("userinfo failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("userinfo returned invalid JSON: {e}"))?;
    let email = userinfo
        .get("email")
        .and_then(Value::as_str)
        .ok_or("userinfo did not include an email")?
        .to_string();
    let name = userinfo
        .get("name")
        .or_else(|| userinfo.get("preferred_username"))
        .and_then(Value::as_str)
        .unwrap_or(&email)
        .to_string();

    // Find or (when the provider allows) create the user.
    let db = &state.core.db;
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM users WHERE lower(email) = lower($1)")
            .bind(&email)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("db: {e}"))?;
    let user_id = match existing {
        Some((id,)) => id,
        None => {
            if !provider.auto_register {
                return Err("no account exists for this identity and auto-registration is disabled".into());
            }
            let password: String = {
                use rand::Rng;
                rand::thread_rng()
                    .sample_iter(&rand::distributions::Alphanumeric)
                    .take(32)
                    .map(char::from)
                    .collect()
            };
            users::create(
                db,
                &users::CreateUser {
                    name,
                    email: email.clone(),
                    password,
                    role: Some("viewer".to_string()),
                },
            )
            .await
            .map_err(|e| format!("could not create user: {e}"))?
            .id
        }
    };

    // An org-scoped provider enrolls the user into that org.
    if let Some(org_id) = provider.org_id {
        orgs::ensure_member(db, org_id, user_id, Role::Viewer)
            .await
            .map_err(|e| format!("org enrollment failed: {e}"))?;
    }

    let user = users::get(db, user_id).await.map_err(|e| format!("db: {e}"))?;
    let jwt = auth::issue_jwt(&state.core.config.jwt_secret, &user)
        .map_err(|e| format!("token issue failed: {e}"))?;
    Ok(format!(
        "{}{}#sso_token={}",
        state.core.config.public_url, sso_state.redirect, jwt
    ))
}

fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
