use axum::{Router, http::header, response::IntoResponse, routing::get};
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub struct MetricsServer {
    registry: Arc<Registry>,
    port: u16,
    cancel: CancellationToken,
}

impl MetricsServer {
    pub fn new(registry: Arc<Registry>, port: u16, cancel: CancellationToken) -> Self {
        Self {
            registry,
            port,
            cancel,
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app = Router::new().route(
            "/metrics",
            get(move || {
                let registry = self.registry.clone();
                async move { metrics_handler(registry).await }
            }),
        );

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("Metrics server listening on http://{}/metrics", addr);

        axum::serve(listener, app)
            .with_graceful_shutdown(self.cancel.cancelled_owned())
            .await?;

        tracing::info!("Metrics server stopped");
        Ok(())
    }
}

async fn metrics_handler(registry: Arc<Registry>) -> impl IntoResponse {
    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        buffer,
    )
}
