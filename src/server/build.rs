//! Build job tracking and output progress parsing.
//! 构建任务跟踪与输出进度解析。

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};

use super::{
    AppState, BuildSnapshot, BuildStatus, ServerConfig, latest_apk_path, lock_or_recover,
    record_event,
};

pub(super) const BUILD_PROGRESS_TOTAL: u64 = 6;

#[derive(Clone, Debug)]
pub(super) struct BuildJob {
    pub(super) id: u64,
    pub(super) status: BuildStatus,
    pub(super) exit_code: Option<i32>,
    pub(super) apk_path: Option<String>,
    pub(super) log: String,
    pub(super) progress_current: u64,
    pub(super) progress_total: u64,
    pub(super) progress_message: String,
}

pub(super) fn queue_build(state: &AppState) -> u64 {
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

    let mut builds = lock_or_recover(&state.inner.builds);
    builds.insert(id, job);
    id
}

pub(super) fn run_build_job(state: AppState, id: u64) {
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
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(("stdout", spawn_output_reader(stdout, sender.clone())));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(("stderr", spawn_output_reader(stderr, sender.clone())));
    }
    drop(sender);

    let mut wait_result = None;
    let mut reader_failed = false;
    while wait_result.is_none() || !readers.is_empty() {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => append_build_output_line(&state, id, &line),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                thread::sleep(Duration::from_millis(50));
            }
        }
        while let Ok(line) = receiver.try_recv() {
            append_build_output_line(&state, id, &line);
        }

        let mut index = 0;
        while index < readers.len() {
            if !readers[index].1.is_finished() {
                index += 1;
                continue;
            }

            let (stream_name, reader) = readers.swap_remove(index);
            if reader.join().unwrap_or(false) {
                continue;
            }

            reader_failed = true;
            append_build_output_line(
                &state,
                id,
                &format!("{stream_name} build output reader panicked"),
            );
            if let Err(error) = child.kill() {
                append_build_output_line(
                    &state,
                    id,
                    &format!("failed to stop build script after reader panic: {error}"),
                );
            }
        }

        if wait_result.is_none() {
            wait_result = match child.try_wait() {
                Ok(Some(status)) => Some(Ok(status)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            };
        }
    }

    while let Ok(line) = receiver.try_recv() {
        append_build_output_line(&state, id, &line);
    }

    let wait_result = wait_result.unwrap_or_else(|| child.wait());
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
    let status = if reader_failed {
        BuildStatus::Failed
    } else {
        status
    };

    record_event(
        &state,
        format!("build #{id} finished status={status:?} exit={exit_code:?}"),
    );
    finish_build_job(&state, id, status, exit_code);
}

fn spawn_output_reader<R>(reader: R, sender: mpsc::Sender<String>) -> JoinHandle<bool>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            read_output_lines(reader, sender);
        }))
        .is_ok()
    })
}

fn read_output_lines<R>(reader: R, sender: mpsc::Sender<String>)
where
    R: Read,
{
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
}

fn finish_build_job(state: &AppState, id: u64, status: BuildStatus, exit_code: Option<i32>) {
    let apk_path = if matches!(status, BuildStatus::Succeeded) {
        Some(latest_apk_path(&state.inner.config.repo_root))
    } else {
        None
    };

    let mut builds = lock_or_recover(&state.inner.builds);
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

pub(super) fn resolve_build_script(config: &ServerConfig) -> PathBuf {
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
    let mut builds = lock_or_recover(&state.inner.builds);
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
    let mut builds = lock_or_recover(&state.inner.builds);
    if let Some(job) = builds.get_mut(&id) {
        job.progress_current = current.min(total);
        job.progress_total = total;
        job.progress_message = message.into();
    }
}

pub(super) fn append_build_output_line(state: &AppState, id: u64, line: &str) {
    let mut progress_event = None;
    {
        let mut builds = lock_or_recover(&state.inner.builds);
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

pub(super) fn snapshot_build(state: &AppState, id: u64) -> Result<BuildSnapshot> {
    let builds = lock_or_recover(&state.inner.builds);
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

pub(super) fn new_build_map() -> BTreeMap<u64, BuildJob> {
    BTreeMap::new()
}
