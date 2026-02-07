use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use p256::ecdsa::SigningKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct KeyRow {
    pub id: i32,
    pub name: String,
    pub key: String,
    pub public_key: String,
    pub domain: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Generate EC P-256 key pair. Returns (private_key_base64, public_key_base64url).
pub fn generate_keypair() -> anyhow::Result<(String, String)> {
    let mut rng = OsRng;
    let signing_key = SigningKey::random(&mut rng);
    let private_bytes = signing_key.to_bytes();
    let public_point = signing_key.verifying_key().to_encoded_point(false);
    let public_bytes = public_point.as_bytes();
    let key_b64 = STANDARD.encode(&private_bytes[..]);
    let public_b64url = URL_SAFE_NO_PAD.encode(public_bytes);
    Ok((key_b64, public_b64url))
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyBody {
    pub name: String,
    pub domain: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateKeyBody {
    pub name: Option<String>,
    pub domain: Option<String>,
}
