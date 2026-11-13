use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tokio::sync::{oneshot, RwLock};

use crate::error::CmdResult;

#[derive(Debug, Clone)]
pub struct ProxyTarget {
    pub url: String,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone)]
struct ProxyContext {
    targets: Arc<RwLock<HashMap<String, ProxyTarget>>>,
    client: reqwest::Client,
}

pub struct ProxyServer {
    pub port: u16,
    targets: Arc<RwLock<HashMap<String, ProxyTarget>>>,
    shutdown: std::sync::Mutex<Option<oneshot::Sender<()>>>,
}

impl ProxyServer {
    pub async fn start() -> CmdResult<Self> {
        let targets: Arc<RwLock<HashMap<String, ProxyTarget>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .no_gzip()
            .build()
            .map_err(crate::error::AppError::Http)?;

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(crate::error::AppError::Io)?;
        let port = listener
            .local_addr()
            .map_err(crate::error::AppError::Io)?
            .port();

        let context = ProxyContext {
            targets: targets.clone(),
            client,
        };
        let router = Router::new()
            .route("/media/{key}", get(handle_media).head(handle_media))
            .with_state(context);

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::serve(listener, router).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });
            if let Err(error) = server.await {
                eprintln!("本地代理退出：{error}");
            }
        });

        Ok(Self {
            port,
            targets,
            shutdown: std::sync::Mutex::new(Some(shutdown_tx)),
        })
    }

    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub async fn register(&self, target: ProxyTarget) -> String {
        let key = uuid::Uuid::new_v4().to_string();
        self.targets.write().await.insert(key.clone(), target);
        format!("{}/media/{}", self.base(), key)
    }

    pub fn stop(&self) {
        if let Some(sender) = self
            .shutdown
            .lock()
            .expect("proxy shutdown lock")
            .take()
        {
            let _ = sender.send(());
        }
    }
}

impl Drop for ProxyServer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn handle_media(
    State(context): State<ProxyContext>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> Response {
    let target = {
        let targets = context.targets.read().await;
        targets.get(&key).cloned()
    };
    let Some(target) = target else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

    let mut request = context.client.get(&target.url);
    if let Some(range) = headers.get(header::RANGE) {
        request = request.header(header::RANGE, range);
    }
    if let Some(referer) = &target.referer {
        request = request.header(header::REFERER, referer);
    }
    if let Some(user_agent) = &target.user_agent {
        request = request.header(header::USER_AGENT, user_agent);
    }

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let mut builder = Response::builder().status(status.as_u16());
            for name in [
                header::CONTENT_TYPE,
                header::CONTENT_LENGTH,
                header::CONTENT_RANGE,
                header::ACCEPT_RANGES,
                header::LAST_MODIFIED,
                header::ETAG,
            ] {
                if let Some(value) = response.headers().get(&name) {
                    builder = builder.header(name, value);
                }
            }
            builder = builder.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");
            let stream = response.bytes_stream();
            builder
                .body(Body::from_stream(stream))
                .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
        }
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            format!("本地代理请求失败：{error}"),
        )
            .into_response(),
    }
}
