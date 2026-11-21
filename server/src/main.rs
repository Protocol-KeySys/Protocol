// protocol-keyserver — authoritative backend for the Protocol key system.
//
// Security model:
//   * Keys are 80-bit random tokens, formatted XXXX-XXXX-XXXX-XXXX-XXXX.
//   * Only SHA-256 hashes of keys are stored — a leaked DB yields nothing usable.
//   * Expiry and platform entitlements live here, never in the client.
//   * Admin issuance requires the PROTOCOL_ADMIN_TOKEN env var (constant-time compare).
//   * Failed validates are rate-limited per IP (simple in-memory counter).

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use tower_http::{cors::CorsLayer, services::ServeDir};

const ALPHABET: &[u8; 16] = b"K7QWZX9MNP3RCTVH";
const KEY_FILE: &str = "keys.json";
const RENEW_SECONDS: u64 = 24 * 3600;
const MAX_FAILS: u32 = 10; // per IP per hour window

// Full catalog shown in the "all apps" view; a key's record lists its subset.
const ALL_PLATFORMS: [&str; 12] = [
    "Platform 1", "Platform 2", "Platform 3", "Platform 4",
    "Platform 5", "Platform 6", "Platform 7", "Platform 8",
    "Platform 9", "Platform 10", "Platform 11", "Platform 12",
];

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn key_hash(key: &str) -> String {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    hex(&h.finalize())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn fmt_key(raw: &[u8; 10]) -> String {
    // 10 bytes = 20 nibbles -> 5 groups of 4 alphabet chars
    let nibbles: Vec<u8> = raw.iter().flat_map(|b| [b >> 4, b & 0xF]).collect();
    nibbles
        .chunks(4)
        .map(|g| g.iter().map(|&n| ALPHABET[n as usize] as char).collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

#[derive(Serialize, Deserialize, Clone)]
struct KeyRecord {
    username: String,
    platforms: Vec<String>,
    expires_at: u64,
}

#[derive(Default)]
struct Store {
    keys: HashMap<String, KeyRecord>, // sha256(key) -> record
    fails: HashMap<String, (u64, u32)>, // ip -> (window_start, count)
}

impl Store {
    fn path() -> PathBuf {
        PathBuf::from(KEY_FILE)
    }

    fn load() -> Self {
        let mut s = Store::default();
        if let Ok(txt) = std::fs::read_to_string(Self::path()) {
            if let Ok(k) = serde_json::from_str(&txt) {
                s.keys = k;
            }
        }
        s
    }

    fn save(&self) {
        if let Ok(txt) = serde_json::to_string_pretty(&self.keys) {
            let _ = std::fs::write(Self::path(), txt);
        }
    }

    fn rate_limited(&mut self, ip: &str) -> bool {
        let t = now();
        let e = self.fails.entry(ip.to_string()).or_insert((t, 0));
        if t - e.0 > 3600 {
            *e = (t, 0);
        }
        e.1 += 1;
        e.1 > MAX_FAILS
    }
}

type Shared = Arc<Mutex<Store>>;

#[derive(Deserialize)]
struct KeyBody {
    key: String,
}

#[derive(Deserialize)]
struct SelectBody {
    key: String,
    platform: String,
}

#[derive(Deserialize)]
struct IssueBody {
    username: String,
    platforms: Vec<String>,
    hours: u64,
}

#[derive(Serialize)]
struct ValidResp {
    valid: bool,
    username: String,
    platforms: Vec<String>,
    seconds_left: u64,
}

fn err(msg: &str) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "valid": false, "ok": false, "error": msg }))
}

fn admin_ok(headers: &HeaderMap) -> bool {
    let expected = std::env::var("PROTOCOL_ADMIN_TOKEN").unwrap_or_default();
    let Some(got) = headers.get("x-admin-token").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    !expected.is_empty()
        && expected.as_bytes().ct_eq(got.as_bytes()).into()
}

async fn validate(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(st): State<Shared>,
    Json(body): Json<KeyBody>,
) -> Result<Json<ValidResp>, (StatusCode, Json<serde_json::Value>)> {
    let ip = addr.ip().to_string();
    let mut s = st.lock().unwrap();
    if s.rate_limited(&ip) {
        return Err((StatusCode::TOO_MANY_REQUESTS, err("rate limited")));
    }
    match s.keys.get(&key_hash(&body.key)) {
        Some(rec) if rec.expires_at > now() => Ok(Json(ValidResp {
            valid: true,
            username: rec.username.clone(),
            platforms: rec.platforms.clone(),
            seconds_left: rec.expires_at - now(),
        })),
        Some(_) => Err((StatusCode::UNAUTHORIZED, err("key expired"))),
        None => Err((StatusCode::UNAUTHORIZED, err("invalid key"))),
    }
}

async fn renew(
    State(st): State<Shared>,
    Json(body): Json<KeyBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut s = st.lock().unwrap();
    let Some(rec) = s.keys.get_mut(&key_hash(&body.key)) else {
        return Err((StatusCode::UNAUTHORIZED, err("invalid key")));
    };
    if rec.expires_at <= now() {
        return Err((StatusCode::UNAUTHORIZED, err("key expired")));
    }
    rec.expires_at += RENEW_SECONDS;
    let left = rec.expires_at - now();
    s.save();
    Ok(Json(serde_json::json!({ "ok": true, "seconds_left": left })))
}

async fn select(
    State(st): State<Shared>,
    Json(body): Json<SelectBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let s = st.lock().unwrap();
    let Some(rec) = s.keys.get(&key_hash(&body.key)) else {
        return Err((StatusCode::UNAUTHORIZED, err("invalid key")));
    };
    if rec.expires_at <= now() {
        return Err((StatusCode::UNAUTHORIZED, err("key expired")));
    }
    if rec.platforms.iter().any(|p| p.eq_ignore_ascii_case(&body.platform)) {
        Ok(Json(serde_json::json!({ "ok": true, "platform": body.platform })))
    } else {
        Err((StatusCode::FORBIDDEN, err("platform not on key")))
    }
}

async fn issue(
    State(st): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<IssueBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !admin_ok(&headers) {
        return Err((StatusCode::FORBIDDEN, err("forbidden")));
    }
    let mut raw = [0u8; 10];
    rand::thread_rng().fill_bytes(&mut raw);
    let key = fmt_key(&raw);

    let rec = KeyRecord {
        username: body.username,
        platforms: body.platforms,
        expires_at: now() + body.hours.min(24 * 365) * 3600,
    };
    st.lock().unwrap().keys.insert(key_hash(&key), rec);
    st.lock().unwrap().save();
    Ok(Json(serde_json::json!({ "key": key })))
}

async fn status(State(st): State<Shared>) -> Json<serde_json::Value> {
    let s = st.lock().unwrap();
    Json(serde_json::json!({ "keys": s.keys.len() }))
}

async fn platforms() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "platforms": ALL_PLATFORMS }))
}

#[tokio::main]
async fn main() {
    let store: Shared = Arc::new(Mutex::new(Store::load()));

    let app = Router::new()
        .route("/api/validate", post(validate))
        .route("/api/renew", post(renew))
        .route("/api/select", post(select))
        .route("/api/admin/issue", post(issue))
        .route("/api/status", get(status))
        .route("/api/platforms", get(platforms))
        .nest_service("/", ServeDir::new("../web"))
        // The packaged client (tauri.localhost) is cross-origin. In prod,
        // restrict this to the client's actual origin instead of Any.
        .layer(CorsLayer::permissive())
        .with_state(store);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("protocol-keyserver listening on http://{addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
