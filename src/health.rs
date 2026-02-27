//! Health check HTTP server.
//!
//! This module provides a simple HTTP server for Kubernetes health probes.
//! It exposes `/healthz` for liveness probes and `/readyz` for readiness probes.

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

/// Health check server for Kubernetes probes.
pub struct HealthServer {
    /// Address to bind the server to.
    addr: SocketAddr,
    /// Whether the controller is ready to accept traffic.
    ready: Arc<AtomicBool>,
}

impl HealthServer {
    /// Create a new health server.
    #[must_use]
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            ready: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a handle to set the ready status.
    #[must_use]
    pub fn ready_handle(&self) -> Arc<AtomicBool> {
        self.ready.clone()
    }

    /// Run the health check server.
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
        _ => {
            let mut response = Response::new(Full::new(Bytes::from("not found")));
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
    }
}
