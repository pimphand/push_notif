use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i32, // user_id
    pub exp: i64,
    pub iat: i64,
}

impl Claims {
    pub fn new(user_id: i32) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            iat: now.timestamp(),
            exp: (now + Duration::days(7)).timestamp(),
        }
    }
}

pub fn create_token(user_id: i32, secret: &[u8]) -> anyhow::Result<String> {
    let claims = Claims::new(user_id);
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )?;
    Ok(token)
}

pub fn decode_token(token: &str, secret: &[u8]) -> anyhow::Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

pub const AUTH_COOKIE_NAME: &str = "auth";

/// Extract user id dari request (cookie auth). Return Unauthorized jika belum login.
pub struct AuthUser(pub i32);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookie_header = parts
            .headers
            .get("Cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let token = cookie::Cookie::split_parse(cookie_header)
            .filter_map(|c| c.ok())
            .find(|c| c.name() == AUTH_COOKIE_NAME)
            .map(|c| c.value().to_string());
        let token = match token {
            Some(t) => t,
            None => return Err((StatusCode::UNAUTHORIZED, "missing auth").into_response()),
        };
        let claims = decode_token(&token, &state.jwt_secret).map_err(|_| {
            (StatusCode::UNAUTHORIZED, "invalid or expired auth").into_response()
        })?;
        Ok(AuthUser(claims.sub))
    }
}
