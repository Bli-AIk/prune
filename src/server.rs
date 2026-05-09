//! HTTP API server for Android prune clients.
//! prune Android 客户端使用的 HTTP API 服务器。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::mods::{ActiveProjectConfig, ModGraph};

mod build;
mod bundle;
#[cfg(test)]
mod tests;

pub use bundle::create_projects_bundle;

use build::{
    BuildJob, new_build_map, queue_build, resolve_build_script, run_build_job, snapshot_build,
};

const MAX_SERVER_EVENTS: usize = 200;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub repo_root: PathBuf,
    pub token: String,
    pub build_script: PathBuf,
}

#[derive(Clone)]
struct AppState {
    inner: Arc<ServerState>,
}

struct ServerState {
    config: ServerConfig,
    next_build_id: AtomicU64,
    builds: Mutex<std::collections::BTreeMap<u64, BuildJob>>,
    events: Mutex<VecDeque<ServerLogEntry>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModsSnapshot {
    pub repository_root: String,
    pub active_mod: String,
    pub active_language: Option<String>,
    pub load_order: Vec<String>,
    pub mods: Vec<ModSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModSnapshot {
    pub name: String,
    pub version: Option<String>,
    pub dependencies: Vec<String>,
    pub runtime_wasm: Option<String>,
    pub content_wasm: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildSnapshot {
    pub id: u64,
    pub status: BuildStatus,
    pub exit_code: Option<i32>,
    pub apk_path: Option<String>,
    pub log: String,
    pub progress_current: u64,
    pub progress_total: u64,
    pub progress_message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerLogEntry {
    pub timestamp_unix_secs: u64,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerInfo {
    pub host: String,
    pub port: u16,
    pub repository_root: String,
    pub build_script: String,
    pub active_mod: Option<String>,
    pub mod_count: Option<usize>,
    pub build_count: usize,
    pub latest_apk_path: String,
    pub latest_apk_exists: bool,
    pub recent_events: Vec<ServerLogEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerLogs {
    pub events: Vec<ServerLogEntry>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuildStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({
            "error": self.message,
        }));
        (self.status, body).into_response()
    }
}

pub async fn run(config: ServerConfig) -> Result<()> {
    let bind = format!("{}:{}", config.host, config.port);
    let state = AppState {
        inner: Arc::new(ServerState {
            config,
            next_build_id: AtomicU64::new(1),
            builds: Mutex::new(new_build_map()),
            events: Mutex::new(VecDeque::new()),
        }),
    };

    let app = router(state);

    let listener = TcpListener::bind(bind)
        .await
        .context("bind prune server listener")?;
    axum::serve(listener, app)
        .await
        .context("serve prune server")
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/info", get(get_info))
        .route("/api/logs", get(get_logs))
        .route("/api/mods", get(get_mods))
        .route("/api/mods/bundle", get(get_bundle))
        .route("/api/builds", post(start_build))
        .route("/api/builds/{id}", get(get_build))
        .route("/api/apk/latest", get(get_latest_apk))
        .with_state(state)
}

async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<HealthResponse>, ApiError> {
    require_auth(&state, &headers)?;
    record_event(&state, "GET /api/health");
    let snapshot = load_mods_snapshot(&state.inner.config.repo_root.join("projects"))
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(HealthResponse {
        ok: true,
        repository_root: state.inner.config.repo_root.display().to_string(),
        active_mod: snapshot.active_mod,
    }))
}

async fn get_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ServerInfo>, ApiError> {
    require_auth(&state, &headers)?;
    record_event(&state, "GET /api/info");
    Ok(Json(
        server_info_snapshot(&state).map_err(|error| ApiError::internal(error.to_string()))?,
    ))
}

async fn get_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ServerLogs>, ApiError> {
    require_auth(&state, &headers)?;
    record_event(&state, "GET /api/logs");
    Ok(Json(ServerLogs {
        events: recent_events(&state),
    }))
}

async fn get_mods(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ModsSnapshot>, ApiError> {
    require_auth(&state, &headers)?;
    record_event(&state, "GET /api/mods");
    let snapshot = load_mods_snapshot(&state.inner.config.repo_root.join("projects"))
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(snapshot))
}

async fn get_bundle(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers)?;
    let projects = state.inner.config.repo_root.join("projects");
    record_event(
        &state,
        format!("GET /api/mods/bundle from {}", projects.display()),
    );
    let mut cursor = std::io::Cursor::new(Vec::new());
    create_projects_bundle(&projects, &mut cursor)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let body = Bytes::from(cursor.into_inner());
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/zip"),
            ),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"projects-bundle.zip\""),
            ),
        ],
        body,
    )
        .into_response())
}

async fn start_build(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<BuildSnapshot>, ApiError> {
    require_auth(&state, &headers)?;
    let id = queue_build(&state);
    record_event(&state, format!("POST /api/builds queued build #{id}"));

    let state_clone = state.clone();
    tokio::task::spawn_blocking(move || run_build_job(state_clone, id));

    Ok(Json(
        snapshot_build(&state, id).map_err(|error| ApiError::internal(error.to_string()))?,
    ))
}

async fn get_build(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<u64>,
) -> Result<Json<BuildSnapshot>, ApiError> {
    require_auth(&state, &headers)?;
    let snapshot =
        snapshot_build(&state, id).map_err(|error| ApiError::not_found(error.to_string()))?;
    record_event(
        &state,
        format!("GET /api/builds/{id} status={:?}", snapshot.status),
    );
    Ok(Json(snapshot))
}

async fn get_latest_apk(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers)?;
    let apk_path = latest_apk_path(&state.inner.config.repo_root);
    record_event(
        &state,
        format!("GET /api/apk/latest from {}", apk_path.display()),
    );
    let bytes = tokio::fs::read(&apk_path).await.map_err(|error| {
        ApiError::not_found(format!("read APK {}: {error}", apk_path.display()))
    })?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/vnd.android.package-archive"),
            ),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"souprune-debug.apk\""),
            ),
        ],
        Bytes::from(bytes),
    )
        .into_response())
}

fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let expected = format!("Bearer {}", state.inner.config.token);
    let actual = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    if actual == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(ApiError::unauthorized("missing or invalid bearer token"))
    }
}

fn load_mods_snapshot(projects_root: &Path) -> Result<ModsSnapshot> {
    let active = ActiveProjectConfig::read(projects_root.join("config.toml"))
        .with_context(|| format!("read {}", projects_root.join("config.toml").display()))?;
    let graph = ModGraph::load(projects_root)?;
    let load_order = graph.dependency_order(&active.mod_name)?;

    let mut mods = Vec::new();
    for (name, manifest) in graph.manifests() {
        mods.push(ModSnapshot {
            name: name.clone(),
            version: manifest.version.clone(),
            dependencies: manifest.dependencies.keys().cloned().collect(),
            runtime_wasm: manifest.mod_library.as_ref().map(|lib| lib.wasm.clone()),
            content_wasm: manifest
                .content_library
                .as_ref()
                .map(|lib| lib.wasm.clone()),
        });
    }

    Ok(ModsSnapshot {
        repository_root: projects_root.display().to_string(),
        active_mod: active.mod_name,
        active_language: active.language,
        load_order,
        mods,
    })
}

fn latest_apk_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join("android")
        .join("souprune")
        .join("build")
        .join("outputs")
        .join("apk")
        .join("debug")
        .join("souprune-debug.apk")
}

fn server_info_snapshot(state: &AppState) -> Result<ServerInfo> {
    let config = &state.inner.config;
    let latest_apk = latest_apk_path(&config.repo_root);
    let snapshot = load_mods_snapshot(&config.repo_root.join("projects")).ok();
    let build_count = state.inner.builds.lock().expect("builds lock").len();

    Ok(ServerInfo {
        host: config.host.clone(),
        port: config.port,
        repository_root: config.repo_root.display().to_string(),
        build_script: resolve_build_script(config).display().to_string(),
        active_mod: snapshot.as_ref().map(|mods| mods.active_mod.clone()),
        mod_count: snapshot.as_ref().map(|mods| mods.mods.len()),
        build_count,
        latest_apk_path: latest_apk.display().to_string(),
        latest_apk_exists: latest_apk.is_file(),
        recent_events: recent_events(state),
    })
}

fn record_event(state: &AppState, message: impl Into<String>) {
    let entry = ServerLogEntry {
        timestamp_unix_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        message: message.into(),
    };
    eprintln!(
        "[prune-server][{}] {}",
        entry.timestamp_unix_secs, entry.message
    );

    let mut events = state.inner.events.lock().expect("events lock");
    events.push_back(entry);
    while events.len() > MAX_SERVER_EVENTS {
        events.pop_front();
    }
}

fn recent_events(state: &AppState) -> Vec<ServerLogEntry> {
    state
        .inner
        .events
        .lock()
        .expect("events lock")
        .iter()
        .cloned()
        .collect()
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    repository_root: String,
    active_mod: String,
}
