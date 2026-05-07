use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
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
use walkdir::WalkDir;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::mods::{ActiveProjectConfig, ModGraph};

const MAX_SERVER_EVENTS: usize = 200;
const BUILD_PROGRESS_TOTAL: u64 = 6;

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
    builds: Mutex<BTreeMap<u64, BuildJob>>,
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

#[derive(Clone, Debug)]
struct BuildJob {
    id: u64,
    status: BuildStatus,
    exit_code: Option<i32>,
    apk_path: Option<String>,
    log: String,
    progress_current: u64,
    progress_total: u64,
    progress_message: String,
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
            builds: Mutex::new(BTreeMap::new()),
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

    let id = state.inner.next_build_id.fetch_add(1, Ordering::SeqCst);
    let job = BuildJob {
        id,
        status: BuildStatus::Queued,
        exit_code: None,
        apk_path: None,
        log: String::new(),
        progress_current: 0,
        progress_total: BUILD_PROGRESS_TOTAL,
        progress_message: "queued".to_string(),
    };

    {
        let mut builds = state.inner.builds.lock().expect("builds lock");
        builds.insert(id, job);
    }
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

fn run_build_job(state: AppState, id: u64) {
    set_build_status(&state, id, BuildStatus::Running, None, None, String::new());
    set_build_progress(&state, id, 0, BUILD_PROGRESS_TOTAL, "running");
    let script = resolve_build_script(&state.inner.config);
    record_event(
        &state,
        format!("build #{id} started: {} --apk-only", script.display()),
    );

    let mut child = match Command::new(&script)
        .arg("--apk-only")
        .current_dir(&state.inner.config.repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            append_build_output_line(&state, id, &format!("failed to run build script: {error}"));
            finish_build_job(&state, id, BuildStatus::Failed, None);
            return;
        }
    };

    let (sender, receiver) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        spawn_output_reader(stdout, sender.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_output_reader(stderr, sender.clone());
    }
    drop(sender);

    for line in receiver {
        append_build_output_line(&state, id, &line);
    }

    let wait_result = child.wait();
    let (status, exit_code) = match wait_result {
        Ok(status) if status.success() => (BuildStatus::Succeeded, status.code()),
        Ok(status) => (BuildStatus::Failed, status.code()),
        Err(error) => {
            append_build_output_line(
                &state,
                id,
                &format!("failed to wait for build script: {error}"),
            );
            (BuildStatus::Failed, None)
        }
    };

    record_event(
        &state,
        format!("build #{id} finished status={status:?} exit={exit_code:?}"),
    );
    finish_build_job(&state, id, status, exit_code);
}

fn spawn_output_reader<R>(reader: R, sender: mpsc::Sender<String>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    let _ = sender.send(line);
                }
                Err(error) => {
                    let _ = sender.send(format!("failed to read build output: {error}"));
                    break;
                }
            }
        }
    });
}

fn finish_build_job(state: &AppState, id: u64, status: BuildStatus, exit_code: Option<i32>) {
    let apk_path = if matches!(status, BuildStatus::Succeeded) {
        Some(latest_apk_path(&state.inner.config.repo_root))
    } else {
        None
    };

    let mut builds = state.inner.builds.lock().expect("builds lock");
    if let Some(job) = builds.get_mut(&id) {
        job.status = status;
        job.exit_code = exit_code;
        job.apk_path = apk_path.as_ref().map(|path| path.display().to_string());
        if matches!(status, BuildStatus::Succeeded) {
            job.progress_current = BUILD_PROGRESS_TOTAL;
            job.progress_total = BUILD_PROGRESS_TOTAL;
            job.progress_message = "build succeeded".to_string();
        } else {
            job.progress_message = "build failed".to_string();
        }
    }
}

fn resolve_build_script(config: &ServerConfig) -> PathBuf {
    if config.build_script.is_absolute() {
        config.build_script.clone()
    } else {
        config.repo_root.join(&config.build_script)
    }
}

fn set_build_status(
    state: &AppState,
    id: u64,
    status: BuildStatus,
    exit_code: Option<i32>,
    apk_path: Option<String>,
    log: String,
) {
    let mut builds = state.inner.builds.lock().expect("builds lock");
    if let Some(job) = builds.get_mut(&id) {
        job.status = status;
        job.exit_code = exit_code;
        job.apk_path = apk_path;
        if !log.is_empty() {
            job.log = log;
        }
    }
}

fn set_build_progress(
    state: &AppState,
    id: u64,
    current: u64,
    total: u64,
    message: impl Into<String>,
) {
    let mut builds = state.inner.builds.lock().expect("builds lock");
    if let Some(job) = builds.get_mut(&id) {
        job.progress_current = current.min(total);
        job.progress_total = total;
        job.progress_message = message.into();
    }
}

fn append_build_output_line(state: &AppState, id: u64, line: &str) {
    let mut progress_event = None;
    {
        let mut builds = state.inner.builds.lock().expect("builds lock");
        if let Some(job) = builds.get_mut(&id) {
            job.log.push_str(line);
            job.log.push('\n');
            if let Some((current, total, message)) = parse_build_progress(line) {
                job.progress_current = current.min(total);
                job.progress_total = total;
                job.progress_message = message.to_string();
                progress_event = Some(format!(
                    "build #{id} progress {}/{} {}",
                    job.progress_current, job.progress_total, job.progress_message
                ));
            }
        }
    }

    if let Some(message) = progress_event {
        record_event(state, message);
    }
}

fn parse_build_progress(line: &str) -> Option<(u64, u64, &'static str)> {
    if line.contains("[assets]") {
        return Some((1, BUILD_PROGRESS_TOTAL, "prepare assets"));
    }
    if line.contains("[1/3]") {
        return Some((2, BUILD_PROGRESS_TOTAL, "build native library"));
    }
    if line.contains("[2/3]") {
        return Some((4, BUILD_PROGRESS_TOTAL, "copy native libraries"));
    }
    if line.contains("[3/3]") {
        return Some((6, BUILD_PROGRESS_TOTAL, "build APK"));
    }
    if line.contains("APK 构建成功") || line.contains("APK build") {
        return Some((BUILD_PROGRESS_TOTAL, BUILD_PROGRESS_TOTAL, "build APK"));
    }
    None
}

fn snapshot_build(state: &AppState, id: u64) -> Result<BuildSnapshot> {
    let builds = state.inner.builds.lock().expect("builds lock");
    let job = builds
        .get(&id)
        .with_context(|| format!("unknown build {id}"))?;
    Ok(BuildSnapshot {
        id: job.id,
        status: job.status,
        exit_code: job.exit_code,
        apk_path: job.apk_path.clone(),
        log: job.log.clone(),
        progress_current: job.progress_current,
        progress_total: job.progress_total,
        progress_message: job.progress_message.clone(),
    })
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

pub fn create_projects_bundle(
    projects_root: impl AsRef<Path>,
    writer: impl Write + Seek,
) -> Result<()> {
    let projects_root = projects_root.as_ref();
    let mut zip = ZipWriter::new(writer);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let prefix = projects_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("projects")
        .to_string();

    for entry in WalkDir::new(projects_root)
        .into_iter()
        .filter_entry(|entry| !should_skip(entry.path()))
    {
        let entry = entry.with_context(|| format!("walk {}", projects_root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(projects_root)
            .with_context(|| format!("strip prefix from {}", path.display()))?;
        if should_skip(relative) {
            continue;
        }

        let archive_path = Path::new(&prefix).join(relative);
        zip.start_file_from_path(&archive_path, options)
            .with_context(|| format!("start zip entry {}", archive_path.display()))?;
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        io::copy(&mut file, &mut zip)
            .with_context(|| format!("write zip entry {}", archive_path.display()))?;
    }

    zip.finish().context("finish zip bundle")?;
    Ok(())
}

fn should_skip(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | ".build" | "target")
        )
    })
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    repository_root: String,
    active_mod: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_builds_without_panicking() {
        let state = AppState {
            inner: Arc::new(ServerState {
                config: ServerConfig {
                    host: "127.0.0.1".to_string(),
                    port: 8788,
                    repo_root: PathBuf::from("/repo"),
                    token: "test-token".to_string(),
                    build_script: PathBuf::from("android/build.sh"),
                },
                next_build_id: AtomicU64::new(1),
                builds: Mutex::new(BTreeMap::new()),
                events: Mutex::new(VecDeque::new()),
            }),
        };

        let result = std::panic::catch_unwind(|| {
            let _ = router(state);
        });

        assert!(result.is_ok());
    }

    #[test]
    fn server_state_records_runtime_events_for_info() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state = AppState {
            inner: Arc::new(ServerState {
                config: ServerConfig {
                    host: "127.0.0.1".to_string(),
                    port: 8788,
                    repo_root: temp.path().to_path_buf(),
                    token: "test-token".to_string(),
                    build_script: PathBuf::from("android/build.sh"),
                },
                next_build_id: AtomicU64::new(1),
                builds: Mutex::new(BTreeMap::new()),
                events: Mutex::new(VecDeque::new()),
            }),
        };

        record_event(&state, "health requested");
        let info = server_info_snapshot(&state).expect("server info");

        assert!(
            info.recent_events
                .iter()
                .any(|event| event.message == "health requested")
        );
    }

    #[test]
    fn latest_apk_path_points_at_souprune_game_apk() {
        let path = latest_apk_path(Path::new("/repo"));
        assert_eq!(
            path,
            PathBuf::from("/repo/android/souprune/build/outputs/apk/debug/souprune-debug.apk")
        );
    }

    #[test]
    fn build_snapshot_serializes_stage_progress() {
        let snapshot = BuildSnapshot {
            id: 7,
            status: BuildStatus::Running,
            exit_code: None,
            apk_path: None,
            log: String::new(),
            progress_current: 3,
            progress_total: 5,
            progress_message: "building native library".to_string(),
        };

        let json = serde_json::to_value(snapshot).expect("serialize build snapshot");

        assert_eq!(json["progress_current"], 3);
        assert_eq!(json["progress_total"], 5);
        assert_eq!(json["progress_message"], "building native library");
    }

    #[test]
    fn build_output_lines_update_stage_progress() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state = AppState {
            inner: Arc::new(ServerState {
                config: ServerConfig {
                    host: "127.0.0.1".to_string(),
                    port: 8788,
                    repo_root: temp.path().to_path_buf(),
                    token: "test-token".to_string(),
                    build_script: PathBuf::from("android/build.sh"),
                },
                next_build_id: AtomicU64::new(1),
                builds: Mutex::new(BTreeMap::from([(
                    1,
                    BuildJob {
                        id: 1,
                        status: BuildStatus::Running,
                        exit_code: None,
                        apk_path: None,
                        log: String::new(),
                        progress_current: 0,
                        progress_total: BUILD_PROGRESS_TOTAL,
                        progress_message: "queued".to_string(),
                    },
                )])),
                events: Mutex::new(VecDeque::new()),
            }),
        };

        append_build_output_line(&state, 1, "▶ [2/3] 复制 .so 到 jniLibs...");
        let snapshot = snapshot_build(&state, 1).expect("snapshot");

        assert_eq!(snapshot.progress_current, 4);
        assert_eq!(snapshot.progress_total, BUILD_PROGRESS_TOTAL);
        assert_eq!(snapshot.progress_message, "copy native libraries");
        assert!(snapshot.log.contains("jniLibs"));
    }
}
