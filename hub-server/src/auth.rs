use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};
use crate::models::SessionPayload;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub org_id: Option<i64>,
    pub exp: usize,
    pub iat: usize,
}

pub fn encode_jwt(session: &SessionPayload, secret: &str) -> Result<String, String> {
    let now = chrono::Utc::now();
    let exp = now + chrono::Duration::hours(24);
    let claims = Claims {
        sub: session.user_id.to_string(),
        username: session.username.clone(),
        display_name: session.display_name.clone(),
        role: session.role.clone(),
        org_id: session.org_id,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| e.to_string())
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<SessionPayload, String> {
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .map_err(|e| e.to_string())?;
    let c = data.claims;
    Ok(SessionPayload {
        user_id: c.sub.parse().map_err(|e: std::num::ParseIntError| e.to_string())?,
        username: c.username,
        display_name: c.display_name,
        role: c.role,
        org_id: c.org_id,
    })
}
