use axum::{
    extract::{Path, State},
    http::{header::SET_COOKIE, HeaderValue, StatusCode},
    response::{AppendHeaders, IntoResponse},
    Json,
};
use serde::Deserialize;
use tracing::{info, warn};
use web_push::SubscriptionInfo;

use crate::auth::{create_token, AuthUser, AUTH_COOKIE_NAME};
use crate::keys::{CreateKeyBody, KeyRow, UpdateKeyBody};
use crate::push_service;
use crate::state::{save_subscriptions, AppState, LastNotification};

#[derive(Deserialize)]
pub struct SubscribeKeys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Deserialize)]
pub struct SubscribeBody {
    pub endpoint: String,
    pub keys: SubscribeKeys,
}

pub async fn vapid_public_key(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "publicKey": state.push_service.public_key_base64url()
    }))
}

pub async fn subscribe(
    State(state): State<AppState>,
    Json(body): Json<SubscribeBody>,
) -> impl IntoResponse {
    let subscription = SubscriptionInfo::new(
        body.endpoint.clone(),
        body.keys.p256dh.clone(),
        body.keys.auth.clone(),
    );
    let endpoint = subscription.endpoint.clone();
    let count = {
        let mut subs = state.subscriptions.write().await;
        subs.add(subscription);
        let to_save = subs.clone();
        if let Err(e) = save_subscriptions(&to_save).await {
            warn!(error = %e, "failed to persist subscriptions");
        }
        subs.len()
    };
    info!(endpoint = %endpoint, count, "subscription added");
    (StatusCode::CREATED, Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NotifyPayload {
    pub title: String,
    pub body: String,
}

pub async fn notify(
    State(state): State<AppState>,
    Json(payload): Json<NotifyPayload>,
) -> impl IntoResponse {
    let subscriptions = {
        let subs = state.subscriptions.read().await;
        subs.all()
    };
    if subscriptions.is_empty() {
        info!("notify called but no subscriptions");
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "sent": 0,
                "failed": 0,
                "message": "No subscriptions"
            })),
        );
    }

    let payload_json = serde_json::json!({
        "title": payload.title,
        "body": payload.body
    });
    let payload_bytes = payload_json.to_string().into_bytes();
    let push_service = state.push_service.clone();
    let total = subscriptions.len();

    let (sent, failed) = push_service::send_to_all(
        &push_service,
        &subscriptions,
        &payload_bytes,
    )
    .await;

    let mut last = state.last_notification.write().await;
    let next_id = last.as_ref().map(|n| n.id + 1).unwrap_or(1);
    *last = Some(LastNotification {
        id: next_id,
        title: payload.title.clone(),
        body: payload.body.clone(),
    });
    let id = next_id;

    info!(sent, failed, total, "notify completed");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "sent": sent,
            "failed": failed,
            "id": id,
            "message": format!("Push terkirim ke {} subscription. Notifikasi akan muncul di browser yang sudah subscribe (browser harus tetap berjalan).", sent)
        })),
    )
}

pub async fn notify_last(State(state): State<AppState>) -> impl IntoResponse {
    let last = state.last_notification.read().await;
    let response = match last.as_ref() {
        Some(n) => serde_json::json!({ "id": n.id, "title": n.title, "body": n.body }),
        None => serde_json::json!({ "id": null, "title": null, "body": null }),
    };
    (StatusCode::OK, Json(response))
}

// --- Auth ---

#[derive(Deserialize)]
pub struct LoginBody {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RegisterBody {
    pub username: String,
    pub password: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> impl IntoResponse {
    let username = body.username.trim();
    if username.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": "Username tidak boleh kosong" })),
        )
            .into_response();
    }
    if username.len() < 3 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": "Username minimal 3 karakter" })),
        )
            .into_response();
    }
    if body.password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": "Password minimal 6 karakter" })),
        )
            .into_response();
    }
    let hash = match bcrypt::hash(&body.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "Gagal mengenkripsi password" })),
            )
                .into_response()
        }
    };
    let result = sqlx::query("INSERT INTO users (username, password_hash) VALUES ($1, $2)")
        .bind(username)
        .bind(&hash)
        .execute(&state.db)
        .await;
    match result {
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "ok": true, "message": "Registrasi berhasil. Silakan login." })),
        )
            .into_response(),
        Err(e) => {
            if let sqlx::Error::Database(ref db) = e {
                if matches!(db.kind(), sqlx::error::ErrorKind::UniqueViolation) {
                    return (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({ "ok": false, "message": "Username sudah dipakai" })),
                    )
                        .into_response();
                }
            }
            tracing::error!(%e, "register insert");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "Gagal mendaftar" })),
            )
                .into_response()
        }
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginBody>,
) -> impl IntoResponse {
    let row: Option<(i32, String)> = sqlx::query_as(
        "SELECT id, password_hash FROM users WHERE username = $1",
    )
    .bind(&body.username)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (user_id, hash) = match row {
        Some(r) => r,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "ok": false, "message": "Username atau password salah" })),
            )
                .into_response()
        }
    };

    if !bcrypt::verify(&body.password, &hash).unwrap_or(false) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "ok": false, "message": "Username atau password salah" })),
        )
            .into_response();
    }

    let token = match create_token(user_id, &state.jwt_secret) {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "Gagal membuat session" })),
            )
                .into_response()
        }
    };

    let cookie = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        AUTH_COOKIE_NAME,
        token,
        7 * 24 * 3600
    );
    let hv = HeaderValue::try_from(cookie.as_str()).unwrap_or(HeaderValue::from_static(""));
    (
        StatusCode::OK,
        AppendHeaders([(SET_COOKIE, hv)]),
        Json(serde_json::json!({ "ok": true, "message": "Login berhasil" })),
    )
        .into_response()
}

pub async fn logout() -> impl IntoResponse {
    let cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        AUTH_COOKIE_NAME
    );
    let hv = HeaderValue::try_from(cookie.as_str()).unwrap_or(HeaderValue::from_static(""));
    (
        StatusCode::OK,
        AppendHeaders([(SET_COOKIE, hv)]),
        Json(serde_json::json!({ "ok": true })),
    )
}

pub async fn me(
    State(state): State<AppState>,
    auth: Result<AuthUser, axum::response::Response>,
) -> impl IntoResponse {
    let AuthUser(user_id) = match auth {
        Ok(a) => a,
        Err(e) => return e,
    };
    let row: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    match row {
        Some((username,)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "username": username })),
        )
            .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "ok": false })),
        )
            .into_response(),
    }
}

// --- Keys (protected) ---

pub async fn keys_list(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
) -> impl IntoResponse {
    let rows: Vec<KeyRow> = sqlx::query_as("SELECT id, name, key, public_key, domain, created_at FROM keys ORDER BY id")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(serde_json::json!(rows)))
}

pub async fn key_create(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Json(body): Json<CreateKeyBody>,
) -> impl IntoResponse {
    let name = body.name.trim();
    let domain = body.domain.trim();
    if name.is_empty() || domain.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": "Nama dan domain wajib diisi" })),
        );
    }
    let (key, public_key) = match crate::keys::generate_keypair() {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!(%e, "generate keypair");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "Gagal generate key" })),
            );
        }
    };
    let row = sqlx::query_as::<_, KeyRow>(
        "INSERT INTO keys (name, key, public_key, domain) VALUES ($1, $2, $3, $4) RETURNING id, name, key, public_key, domain, created_at",
    )
    .bind(name)
    .bind(&key)
    .bind(&public_key)
    .bind(domain)
    .fetch_one(&state.db)
    .await;
    match row {
        Ok(r) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "ok": true, "key": r })),
        ),
        Err(e) => {
            tracing::error!(%e, "insert key");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "Gagal menyimpan key" })),
            )
        }
    }
}

pub async fn key_update(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdateKeyBody>,
) -> impl IntoResponse {
    let existing: Option<KeyRow> = sqlx::query_as("SELECT id, name, key, public_key, domain, created_at FROM keys WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    let mut row = match existing {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "ok": false, "message": "Key tidak ditemukan" })),
            );
        }
    };
    if let Some(n) = body.name.as_deref() {
        if !n.trim().is_empty() {
            row.name = n.trim().to_string();
        }
    }
    if let Some(d) = body.domain.as_deref() {
        if !d.trim().is_empty() {
            row.domain = d.trim().to_string();
        }
    }
    let updated = sqlx::query("UPDATE keys SET name = $1, domain = $2 WHERE id = $3")
        .bind(&row.name)
        .bind(&row.domain)
        .bind(id)
        .execute(&state.db)
        .await;
    match updated {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "key": row })),
        ),
        Err(e) => {
            tracing::error!(%e, "update key");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "Gagal update key" })),
            )
        }
    }
}

pub async fn key_regenerate(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let (key, public_key) = match crate::keys::generate_keypair() {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!(%e, "regenerate keypair");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "Gagal generate key" })),
            );
        }
    };
    let result = sqlx::query("UPDATE keys SET key = $1, public_key = $2 WHERE id = $3")
        .bind(&key)
        .bind(&public_key)
        .bind(id)
        .execute(&state.db)
        .await;
    match result {
        Ok(r) if r.rows_affected() > 0 => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "message": "Key dan Public Key berhasil diregenerate" })),
        ),
        _ => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "ok": false, "message": "Key tidak ditemukan" })),
        ),
    }
}

pub async fn key_delete(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM keys WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await;
    match result {
        Ok(r) if r.rows_affected() > 0 => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true })),
        ),
        _ => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "ok": false, "message": "Key tidak ditemukan" })),
        ),
    }
}
