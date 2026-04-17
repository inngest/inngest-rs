#![allow(dead_code)]

use axum::Router;
use inngest::{client::Inngest, handler::Handler, serve};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs, io,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const DEV_SERVER_ORIGIN: &str = "http://127.0.0.1:8288";

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    data: T,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EventRunRecord {
    pub run_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunRecord {
    pub status: String,
    #[serde(default)]
    pub output: Value,
}

pub struct AppServer {
    base_url: String,
    task: tokio::task::JoinHandle<()>,
}

impl AppServer {
    pub async fn sync(&self) {
        // Sync the SDK app with the dev server after the local axum app is listening.
        let response = reqwest::Client::new()
            .put(format!("{}/api/inngest", self.base_url))
            .send()
            .await
            .expect("sync request should complete");
        let status = response.status();
        let body = response
            .text()
            .await
            .expect("sync response body should be readable");

        assert!(
            status.is_success(),
            "sync request failed with status {}",
            status
        );

        serde_json::from_str::<Value>(&body).unwrap_or_else(|_| {
            panic!("sync request returned a non-JSON body: {body}");
        });
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct DevServer {
    child: Child,
}

impl DevServer {
    pub async fn start() -> Self {
        // Each e2e test owns the single shared dev server while holding the lock below.
        let child = Command::new("inngest")
            .args(["dev", "--no-discovery", "--no-poll", "--port", "8288"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("inngest dev server should start");

        wait_for_http_ok(DEV_SERVER_ORIGIN, Duration::from_secs(20)).await;

        Self { child }
    }
}

impl Drop for DevServer {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }

        let _ = self.child.wait();
    }
}

pub struct DevServerLock {
    path: PathBuf,
}

impl DevServerLock {
    pub fn acquire() -> Self {
        // The dev server uses fixed ports, so keep the e2e suite serialized.
        let path = std::env::temp_dir().join("inngest-rs-dev-server-e2e.lock");
        let deadline = Instant::now() + Duration::from_secs(30);

        loop {
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    assert!(
                        Instant::now() < deadline,
                        "timed out waiting for dev server lock"
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(err) => panic!("failed to create dev server lock: {err}"),
            }
        }
    }
}

impl Drop for DevServerLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

pub async fn spawn_app(client: Inngest, funcs: Vec<inngest::handler::RegisteredFn>) -> AppServer {
    // Give each test app its own ephemeral HTTP server and sync only the functions it needs.
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have a local address");
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    let mut handler = Handler::new(&client).serve_origin(&base_url);
    handler.register_fns(funcs);

    let app = Router::new()
        .route(
            "/api/inngest",
            axum::routing::get(serve::axum::introspect)
                .put(serve::axum::register)
                .post(serve::axum::invoke),
        )
        .with_state(Arc::new(handler));

    let task = tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .expect("server should bind")
            .serve(app.into_make_service())
            .await
            .expect("server should serve");
    });

    AppServer { base_url, task }
}

pub async fn wait_for_run_status(
    run_id: &str,
    expected_status: &str,
    timeout: Duration,
) -> RunRecord {
    // The dev server updates runs asynchronously, so poll until the desired state appears.
    let deadline = Instant::now() + timeout;
    let mut last_status = None::<String>;

    loop {
        if let Some(run) = fetch_run(run_id).await {
            if run.status == expected_status {
                return run;
            }
            last_status = Some(run.status);
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for run {run_id} to reach status {expected_status}; last status was {:?}",
            last_status
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn wait_for_event_runs(event_id: &str, timeout: Duration) -> Vec<EventRunRecord> {
    // Child runs may not exist immediately after the parent emits the durable event.
    let deadline = Instant::now() + timeout;

    loop {
        let runs = fetch_event_runs(event_id).await;
        if !runs.is_empty() {
            return runs;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for event {event_id} to produce runs"
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn wait_for_state<T: Clone>(state: &Arc<Mutex<Option<T>>>, timeout: Duration) -> T {
    // Test functions communicate back to the harness through shared in-memory state.
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(value) = state.lock().unwrap().clone() {
            return value;
        }

        assert!(Instant::now() < deadline, "timed out waiting for state");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub fn unique_name(prefix: &str) -> String {
    // Reusing names across tests makes dev-server sync confusing, so keep them unique.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();

    format!("{prefix}-{now}")
}

async fn wait_for_http_ok(origin: &str, timeout: Duration) {
    // The CLI can take a moment to boot before its HTTP endpoints are ready.
    let deadline = Instant::now() + timeout;
    let client = reqwest::Client::new();

    loop {
        if let Ok(response) = client.get(origin).send().await {
            if response.status().is_success() {
                return;
            }
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for {origin} to become healthy"
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn fetch_run(run_id: &str) -> Option<RunRecord> {
    // Missing runs are normal while the event is still being ingested.
    let response = reqwest::Client::new()
        .get(format!("{DEV_SERVER_ORIGIN}/v1/runs/{run_id}"))
        .send()
        .await
        .expect("run lookup should complete");

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return None;
    }

    assert!(
        response.status().is_success(),
        "run lookup failed with status {}",
        response.status()
    );

    let envelope = response
        .json::<ApiEnvelope<RunRecord>>()
        .await
        .expect("run lookup should decode");

    Some(envelope.data)
}

async fn fetch_event_runs(event_id: &str) -> Vec<EventRunRecord> {
    // Child event runs appear only after the durable event has been dispatched.
    let response = reqwest::Client::new()
        .get(format!("{DEV_SERVER_ORIGIN}/v1/events/{event_id}/runs"))
        .send()
        .await
        .expect("event run lookup should complete");

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Vec::new();
    }

    assert!(
        response.status().is_success(),
        "event run lookup failed with status {}",
        response.status()
    );

    let envelope = response
        .json::<ApiEnvelope<Vec<EventRunRecord>>>()
        .await
        .expect("event run lookup should decode");

    envelope.data
}
