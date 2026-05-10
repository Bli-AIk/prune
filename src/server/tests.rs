//! Tests for prune server state and build progress helpers.
//! prune 服务器状态与构建进度辅助逻辑测试。

use super::*;
use crate::server::build::{
    BUILD_PROGRESS_TOTAL, BuildJob, append_build_output_line, snapshot_build,
};

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
            builds: Mutex::new(new_build_map()),
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
            builds: Mutex::new(new_build_map()),
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
fn server_state_recovers_poisoned_event_lock() {
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
            builds: Mutex::new(new_build_map()),
            events: Mutex::new(VecDeque::new()),
        }),
    };

    let poisoned = state.clone();
    let _ = std::panic::catch_unwind(move || {
        let _guard = poisoned.inner.events.lock().expect("events lock");
        panic!("poison events lock");
    });

    record_event(&state, "recovered");

    assert!(
        recent_events(&state)
            .iter()
            .any(|event| event.message == "recovered")
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
            builds: Mutex::new(std::collections::BTreeMap::from([(
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
