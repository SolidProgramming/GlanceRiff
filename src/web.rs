use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::Engine;
use serde_json::{json, Value};

use crate::{
    collector::Collector,
    history::{History, HistoryPoint},
    model::Snapshot,
};

struct AppState {
    snapshot: RwLock<Snapshot>,
    history: RwLock<History>,
}

type Shared = Arc<AppState>;

const INDEX_HTML: &str = include_str!("../static/index.html");

/// Accepted credentials. A request passes if it matches any of them.
pub struct Auth {
    /// `(scheme, credentials)`, e.g. `("Basic", "dXNlcjpwYXNz")`.
    accepted: Vec<(&'static str, String)>,
}

impl Auth {
    /// Reads `GLANCERIFF_USERNAME`, `GLANCERIFF_PASSWORD` and `GLANCERIFF_TOKEN`.
    /// Empty variables count as unset, so a blank container field means "off".
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
        Self::from_vars(var("GLANCERIFF_USERNAME"), var("GLANCERIFF_PASSWORD"), var("GLANCERIFF_TOKEN"))
    }

    fn from_vars(
        username: Option<String>,
        password: Option<String>,
        token: Option<String>,
    ) -> anyhow::Result<Option<Self>> {
        let basic = match (username, password) {
            (Some(user), _) if user.contains(':') => {
                anyhow::bail!("GLANCERIFF_USERNAME must not contain ':'")
            }
            (Some(user), Some(pass)) => Some((user, pass)),
            (None, None) => None,
            _ => anyhow::bail!("set both GLANCERIFF_USERNAME and GLANCERIFF_PASSWORD, or neither"),
        };
        Ok(Self::new(basic.as_ref().map(|(u, p)| (u.as_str(), p.as_str())), token.as_deref()))
    }

    /// Returns `None` if neither is set.
    fn new(basic: Option<(&str, &str)>, token: Option<&str>) -> Option<Self> {
        let mut accepted = Vec::new();
        if let Some((user, pass)) = basic {
            let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
            accepted.push(("Basic", encoded));
        }
        if let Some(token) = token {
            accepted.push(("Bearer", token.to_string()));
        }
        (!accepted.is_empty()).then_some(Self { accepted })
    }

    fn authorized(&self, headers: &HeaderMap) -> bool {
        let Some((scheme, creds)) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().split_once(' '))
        else {
            return false;
        };
        let creds = creds.trim();
        // Check every entry so the timing doesn't reveal which one matched.
        self.accepted.iter().fold(false, |ok, (s, expected)| {
            ok | (s.eq_ignore_ascii_case(scheme) & ct_eq(expected.as_bytes(), creds.as_bytes()))
        })
    }
}

/// Compares in time independent of where the inputs differ.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn require_auth(State(auth): State<Arc<Auth>>, req: Request, next: Next) -> Response {
    if auth.authorized(req.headers()) {
        return next.run(req).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, r#"Basic realm="GlanceRiff", charset="UTF-8""#)],
        "unauthorized",
    )
        .into_response()
}

pub fn run(
    bind: SocketAddr,
    interval: Duration,
    normalize_cpu: bool,
    auth: Option<Auth>,
) -> anyhow::Result<()> {
    let mut collector = Collector::new();
    let first = collector.collect();
    let mut history = History::new(interval);
    history.push(&first);
    // The dashboard polls at the collection interval and keeps as many points as the server.
    let config = json!({
        "normalize_cpu": normalize_cpu,
        "interval_ms": interval.as_millis() as u64,
        "history_len": history.capacity(),
    });
    let shared: Shared = Arc::new(AppState {
        snapshot: RwLock::new(first),
        history: RwLock::new(history),
    });

    // sysinfo refreshes are blocking, so collect on a plain thread.
    let writer = shared.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(interval);
        let snapshot = collector.collect();
        writer.history.write().unwrap().push(&snapshot);
        *writer.snapshot.write().unwrap() = snapshot;
    });

    let mut app = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/api/snapshot", get(snapshot))
        .route("/api/history", get(history_points))
        .route("/api/config", get(move || async move { Json(config) }))
        .route("/api/host", get(|s: State<Shared>| part(s, |s| json!(s.host))))
        .route("/api/cpu", get(|s: State<Shared>| part(s, |s| json!(s.cpu))))
        .route("/api/mem", get(|s: State<Shared>| part(s, |s| json!({ "mem": s.mem, "swap": s.swap }))))
        .route("/api/disks", get(|s: State<Shared>| part(s, |s| json!(s.disks))))
        .route("/api/net", get(|s: State<Shared>| part(s, |s| json!(s.nets))))
        .route("/api/sensors", get(|s: State<Shared>| part(s, |s| json!(s.sensors))))
        .route("/api/processes", get(|s: State<Shared>| part(s, |s| json!(s.processes))))
        .with_state(shared);

    let protected = auth.is_some();
    if let Some(auth) = auth {
        app = app.layer(middleware::from_fn_with_state(Arc::new(auth), require_auth));
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::bind(bind).await?;
        println!("GlanceRiff web UI on http://{bind}  (Ctrl+C to stop)");
        if protected {
            println!("authentication required");
        } else if !bind.ip().is_loopback() {
            println!(
                "warning: reachable from the network without authentication \
                 (set GLANCERIFF_USERNAME and GLANCERIFF_PASSWORD)"
            );
        }
        axum::serve(listener, app).await?;
        Ok(())
    })
}

async fn snapshot(State(shared): State<Shared>) -> Json<Snapshot> {
    Json(shared.snapshot.read().unwrap().clone())
}

async fn history_points(State(shared): State<Shared>) -> Json<Vec<HistoryPoint>> {
    Json(shared.history.read().unwrap().to_vec())
}

async fn part(State(shared): State<Shared>, pick: impl FnOnce(&Snapshot) -> Value) -> Json<Value> {
    Json(pick(&shared.snapshot.read().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        h
    }

    // "admin:secret" in base64.
    const ADMIN_SECRET: &str = "YWRtaW46c2VjcmV0";

    #[test]
    fn none_without_credentials() {
        assert!(Auth::new(None, None).is_none());
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn from_vars() {
        assert!(Auth::from_vars(None, None, None).unwrap().is_none());
        assert!(Auth::from_vars(some("admin"), some("secret"), None).unwrap().is_some());
        assert!(Auth::from_vars(None, None, some("abc")).unwrap().is_some());
        // Half a login is a configuration mistake, not "no login".
        assert!(Auth::from_vars(some("admin"), None, None).is_err());
        assert!(Auth::from_vars(None, some("secret"), some("abc")).is_err());
        assert!(Auth::from_vars(some("ad:min"), some("secret"), None).is_err());
        // A colon in the password is fine.
        let auth = Auth::from_vars(some("admin"), some("se:cret"), None).unwrap().unwrap();
        assert!(auth.authorized(&headers("Basic YWRtaW46c2U6Y3JldA=="))); // admin:se:cret
    }

    #[test]
    fn basic() {
        let auth = Auth::new(Some(("admin", "secret")), None).unwrap();
        assert!(auth.authorized(&headers(&format!("Basic {ADMIN_SECRET}"))));
        assert!(auth.authorized(&headers(&format!("basic {ADMIN_SECRET}"))));
        assert!(!auth.authorized(&headers("Basic YWRtaW46d3Jvbmc="))); // admin:wrong
        assert!(!auth.authorized(&headers(&format!("Bearer {ADMIN_SECRET}"))));
        assert!(!auth.authorized(&HeaderMap::new()));
    }

    #[test]
    fn bearer() {
        let auth = Auth::new(None, Some("abc")).unwrap();
        assert!(auth.authorized(&headers("Bearer abc")));
        assert!(!auth.authorized(&headers("Bearer abd")));
        assert!(!auth.authorized(&headers("Bearer ab")));
        assert!(!auth.authorized(&headers("Basic abc")));
        assert!(!auth.authorized(&headers("abc")));
    }

    #[test]
    fn either_method_when_both_set() {
        let auth = Auth::new(Some(("admin", "secret")), Some("abc")).unwrap();
        assert!(auth.authorized(&headers(&format!("Basic {ADMIN_SECRET}"))));
        assert!(auth.authorized(&headers("Bearer abc")));
        assert!(!auth.authorized(&headers("Bearer nope")));
    }
}
