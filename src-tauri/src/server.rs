use crate::AppState;
use crate::db::{ChangeResult, DbError, DbResult, IncomingChange, PullResult};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

fn now_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

// ---------- pairing codes (kept in memory only) ----------

struct Code {
    value: String,
    expires_at: i64,
    attempts_left: u8,
}

#[derive(Default)]
pub struct Pairing(Mutex<Option<Code>>);

impl Pairing {
    /// Makes a new 6-digit code, valid for 10 minutes. Replaces any older one.
    pub fn start(&self) -> DbResult<String> {
        let n = u128::from_le_bytes(*uuid::Uuid::new_v4().as_bytes()) % 1_000_000;
        let value = format!("{n:06}");
        let mut slot = self.lock()?;
        *slot = Some(Code {
            value: value.clone(),
            expires_at: now_ms() + 10 * 60 * 1000,
            attempts_left: 5,
        });
        Ok(value)
    }

    /// Checks a code. A right code works once; five wrong guesses burn it.
    pub fn consume(&self, given: &str) -> DbResult<()> {
        let mut slot = self.lock()?;
        let Some(code) = slot.as_mut() else {
            return Err(DbError::PairingFailed);
        };

        if now_ms() > code.expires_at {
            *slot = None;
            return Err(DbError::PairingFailed);
        }
        if code.value == given.trim() {
            *slot = None;
            return Ok(());
        }
        code.attempts_left -= 1;
        if code.attempts_left == 0 {
            *slot = None;
        }
        Err(DbError::PairingFailed)
    }

    fn lock(&self) -> DbResult<std::sync::MutexGuard<'_, Option<Code>>> {
        self.0
            .lock()
            .map_err(|_| DbError::Internal("pairing lock poisoned".into()))
    }
}

// ---------- turning errors into HTTP answers ----------

struct ApiError(DbError);

impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            DbError::Unauthorized => StatusCode::UNAUTHORIZED,
            DbError::PairingFailed => StatusCode::FORBIDDEN,
            DbError::Sqlite(_) | DbError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.0.to_string() })),
        )
            .into_response()
    }
}

// ---------- the routes ----------

pub async fn run(app: AppHandle) -> std::io::Result<()> {
    let router = Router::new()
        .route("/pair", post(pair))
        .route("/sync/push", post(push))
        .route("/sync/pull", get(pull))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8787").await?;
    axum::serve(listener, router).await
}

fn bearer(headers: &HeaderMap) -> Result<&str, DbError> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(DbError::Unauthorized)
}

#[derive(Deserialize)]
struct PairRequest {
    code: String,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairResponse {
    device_id: String,
    token: String,
}

async fn pair(
    State(app): State<AppHandle>,
    Json(req): Json<PairRequest>,
) -> Result<Json<PairResponse>, ApiError> {
    app.state::<Pairing>().consume(&req.code)?;
    let state = app.state::<AppState>();
    let db = state.db()?;
    let (device_id, token) = db.create_device(req.name.trim())?;
    Ok(Json(PairResponse { device_id, token }))
}

#[derive(Deserialize)]
struct PushRequest {
    changes: Vec<IncomingChange>,
}

#[derive(Serialize)]
struct PushResponse {
    results: Vec<ChangeResult>,
}

async fn push(
    State(app): State<AppHandle>,
    headers: HeaderMap,
    Json(req): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    let state = app.state::<AppState>();
    let mut db = state.db()?;
    let device_id = db.authenticate(bearer(&headers)?)?;
    let results = db.apply_changes(&device_id, &req.changes)?;
    Ok(Json(PushResponse { results }))
}

#[derive(Deserialize)]
struct PullQuery {
    since: Option<i64>,
    limit: Option<i64>,
}

async fn pull(
    State(app): State<AppHandle>,
    headers: HeaderMap,
    Query(q): Query<PullQuery>,
) -> Result<Json<PullResult>, ApiError> {
    let state = app.state::<AppState>();
    let db = state.db()?;
    db.authenticate(bearer(&headers)?)?;
    Ok(Json(db.changes_since(
        q.since.unwrap_or(0),
        q.limit.unwrap_or(200),
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_code_is_single_use_and_limited() {
        let p = Pairing::default();
        let code = p.start().unwrap();
        assert_eq!(code.len(), 6);

        assert!(p.consume("not-it").is_err());
        assert!(p.consume(&code).is_ok()); // the right code works once...
        assert!(p.consume(&code).is_err()); // ...and only once

        let code = p.start().unwrap();
        let wrong = if code == "111111" { "222222" } else { "111111" };
        for _ in 0..5 {
            assert!(p.consume(wrong).is_err());
        }
        assert!(p.consume(&code).is_err()); // five wrong guesses burned it
    }
}
