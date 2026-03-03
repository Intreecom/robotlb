use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use http_body_util::Full;
use hyper::{
    body::Bytes,
    server::conn::http1,
    service::service_fn,
    {Request, Response, StatusCode},
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub struct HealthServer {
    addr: SocketAddr,
    ready: Arc<AtomicBool>,
}

impl HealthServer {
    #[must_use]
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            ready: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    pub fn ready_handle(&self) -> Arc<AtomicBool> {
        self.ready.clone()
    }

    pub async fn run(self, shutdown: CancellationToken) {
        let listener = match TcpListener::bind(self.addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind health check server: {}", e);
                return;
            }
        };

        tracing::info!("Health check server listening on {}", self.addr);

        loop {
            tokio::select! {
                () = shutdown.cancelled() => {
                    tracing::info!("Health check server shutting down");
                    break;
                }
                accept = listener.accept() => {
                    match accept {
                        Ok((stream, _)) => {
                            let ready = self.ready.clone();
                            let io = TokioIo::new(stream);
                            tokio::spawn(async move {
                                let service = service_fn(move |req: Request<hyper::body::Incoming>| {
                                    let ready = ready.clone();
                                    std::future::ready(Ok::<_, std::convert::Infallible>(handle_request(&req, &ready)))
                                });
                                if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                                    tracing::debug!("Health check connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::debug!("Failed to accept connection: {}", e);
                        }
                    }
                }
            }
        }
    }
}

fn handle_request(
    req: &Request<hyper::body::Incoming>,
    ready: &Arc<AtomicBool>,
) -> Response<Full<Bytes>> {
    let path = req.uri().path();

    match path {
        "/healthz" => Response::new(Full::new(Bytes::from("ok"))),
        "/readyz" => {
            if ready.load(Ordering::Relaxed) {
                Response::new(Full::new(Bytes::from("ready")))
            } else {
                let mut response = Response::new(Full::new(Bytes::from("not ready")));
                *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
                response
            }
        }
        "/metrics" => {
            match crate::prometheus_exporter::format_prometheus_metrics() {
                Ok(metrics) => {
                    let mut response = Response::new(Full::new(Bytes::from(metrics)));
                    response.headers_mut().insert(
                        hyper::header::CONTENT_TYPE,
                        hyper::header::HeaderValue::from_static(
                            "application/openmetrics-text; version=1.0.0; charset=utf-8",
                        ),
                    );
                    response
                }
                Err(e) => {
                    tracing::error!("Failed to get metrics: {}", e);
                    let mut response =
                        Response::new(Full::new(Bytes::from(format!("Error: {}", e))));
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    response
                }
            }
        }
        _ => {
            let mut response = Response::new(Full::new(Bytes::from("not found")));
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
    }
}
