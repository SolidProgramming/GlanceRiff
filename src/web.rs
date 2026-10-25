use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use axum::{extract::State, response::Html, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{collector::Collector, model::Snapshot};

type Shared = Arc<RwLock<Snapshot>>;

const INDEX_HTML: &str = include_str!("../static/index.html");

pub fn run(bind: SocketAddr, interval: Duration, normalize_cpu: bool) -> anyhow::Result<()> {
    let mut collector = Collector::new();
    let shared: Shared = Arc::new(RwLock::new(collector.collect()));

    // sysinfo refreshes are blocking, so collect on a plain thread.
    let writer = shared.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(interval);
        let snapshot = collector.collect();
        *writer.write().unwrap() = snapshot;
    });

    let app = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/api/snapshot", get(snapshot))
        .route("/api/config", get(move || async move { Json(json!({ "normalize_cpu": normalize_cpu })) }))
        .route("/api/host", get(|s: State<Shared>| part(s, |s| json!(s.host))))
        .route("/api/cpu", get(|s: State<Shared>| part(s, |s| json!(s.cpu))))
        .route("/api/mem", get(|s: State<Shared>| part(s, |s| json!({ "mem": s.mem, "swap": s.swap }))))
        .route("/api/disks", get(|s: State<Shared>| part(s, |s| json!(s.disks))))
        .route("/api/net", get(|s: State<Shared>| part(s, |s| json!(s.nets))))
        .route("/api/processes", get(|s: State<Shared>| part(s, |s| json!(s.processes))))
        .with_state(shared);

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::bind(bind).await?;
        println!("GlanceRiff web UI on http://{bind}  (Ctrl+C to stop)");
        if !bind.ip().is_loopback() {
            println!("warning: reachable from the network without authentication");
        }
        axum::serve(listener, app).await?;
        Ok(())
    })
}

async fn snapshot(State(shared): State<Shared>) -> Json<Snapshot> {
    Json(shared.read().unwrap().clone())
}

async fn part(State(shared): State<Shared>, pick: impl FnOnce(&Snapshot) -> Value) -> Json<Value> {
    Json(pick(&shared.read().unwrap()))
}
