//! `RobotLB` - A Kubernetes load balancer operator for Hetzner Cloud.
//!
//! This operator manages Hetzner Cloud load balancers for Kubernetes services
//! with the `robotlb` load balancer class.
//!
//! # Features
//!
//! - Automatic load balancer creation and management
//! - Support for multiple load balancer types and algorithms
//! - Health check configuration
//! - Proxy protocol support
//! - Private networking support
//! - Leader election for high availability
//! - Health check endpoints (`/healthz`, `/readyz`)

#![warn(
    // Base lints.
    clippy::all,
    // Some pedantic lints.
    clippy::pedantic,
    // New lints which are cool.
    clippy::nursery,
)]

use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use clap::Parser;
use hcloud::apis::configuration::Configuration as HCloudConfig;
use kube_leader_election::{LeaseLock, LeaseLockParams, LeaseLockResult};
use tokio_util::sync::CancellationToken;

use crate::{config::OperatorConfig, controller::run, error::RobotLBResult, health::HealthServer};

pub mod config;
pub mod consts;
pub mod controller;
pub mod error;
pub mod finalizers;
pub mod health;
pub mod label_filter;
pub mod lb;

/// Shared context for the operator.
#[derive(Clone)]
pub struct CurrentContext {
    /// Kubernetes client.
    pub client: kube::Client,
    /// Operator configuration.
    pub config: OperatorConfig,
    /// Hetzner Cloud API configuration.
    pub hcloud_config: HCloudConfig,
    /// Leader election status.
    pub is_leader: Arc<AtomicBool>,
}

impl CurrentContext {
    /// Create a new context.
    #[must_use]
    pub const fn new(
        client: kube::Client,
        config: OperatorConfig,
        hcloud_config: HCloudConfig,
        is_leader: Arc<AtomicBool>,
    ) -> Self {
        Self {
            client,
            config,
            hcloud_config,
            is_leader,
        }
    }
}

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> RobotLBResult<()> {
    dotenvy::dotenv().ok();
    let operator_config = OperatorConfig::parse();
    tracing_subscriber::fmt()
        .with_max_level(operator_config.log_level)
        .init();

    let mut hcloud_conf = HCloudConfig::new();
    hcloud_conf.bearer_access_token = Some(operator_config.hcloud_token.clone());

    tracing::info!("Starting robotlb operator v{}", env!("CARGO_PKG_VERSION"));
    let kube_client = kube::Client::try_default().await?;
    tracing::info!("Kube client is connected");

    let shutdown_token = CancellationToken::new();
    let is_leader = Arc::new(AtomicBool::new(false));

    // Start health check server
    let health_addr: SocketAddr = "0.0.0.0:8080".parse().expect("valid address");
    let health_server = HealthServer::new(health_addr);
    let ready_handle = health_server.ready_handle();
    let health_shutdown = shutdown_token.clone();
    tokio::spawn(async move {
        health_server.run(health_shutdown).await;
    });

    spawn_leader_election_task(
        kube_client.clone(),
        &operator_config,
        is_leader.clone(),
        shutdown_token.clone(),
    );

    let context = Arc::new(CurrentContext::new(
        kube_client.clone(),
        operator_config.clone(),
        hcloud_conf,
        is_leader.clone(),
    ));
    tracing::info!("Starting the controller");

    // Mark as ready once controller starts
    ready_handle.store(true, Ordering::Relaxed);

    let controller_shutdown = shutdown_token.clone();
    let controller_task = tokio::spawn(async move {
        run(kube_client, context, controller_shutdown).await;
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal");
            shutdown_token.cancel();
        }
        _ = controller_task => {}
    }

    tracing::info!("Shutdown complete");
    Ok(())
}

fn spawn_leader_election_task(
    client: kube::Client,
    config: &OperatorConfig,
    is_leader: Arc<AtomicBool>,
    shutdown: CancellationToken,
) {
    let lease_namespace = config
        .leader_election_namespace
        .clone()
        .unwrap_or_else(detect_runtime_namespace);
    let holder_id =
        std::env::var("HOSTNAME").unwrap_or_else(|_| format!("robotlb-{}", std::process::id()));
    let lease = LeaseLock::new(
        client,
        &lease_namespace,
        LeaseLockParams {
            holder_id: holder_id.clone(),
            lease_name: config.leader_election_lease_name.clone(),
            lease_ttl: Duration::from_secs(config.leader_election_lease_ttl_secs),
        },
    );
    let renew_interval = Duration::from_secs(config.leader_election_renew_interval_secs);

    tracing::info!(
        lease_name = %config.leader_election_lease_name,
        lease_namespace = %lease_namespace,
        holder_id = %holder_id,
        "Starting leader election"
    );

    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = shutdown.cancelled() => {
                    tracing::info!("Leader election task shutting down");
                    break;
                }
                result = lease.try_acquire_or_renew() => {
                    match result {
                        Ok(LeaseLockResult::Acquired(_)) => {
                            if !is_leader.load(Ordering::Relaxed) {
                                tracing::info!("Leadership acquired");
                            }
                            is_leader.store(true, Ordering::Relaxed);
                        }
                        Ok(LeaseLockResult::NotAcquired(_)) => {
                            if is_leader.load(Ordering::Relaxed) {
                                tracing::warn!("Leadership lost");
                            }
                            is_leader.store(false, Ordering::Relaxed);
                        }
                        Err(err) => {
                            if is_leader.load(Ordering::Relaxed) {
                                tracing::warn!(error = %err, "Leader election failed, stepping down");
                            } else {
                                tracing::warn!(error = %err, "Leader election attempt failed");
                            }
                            is_leader.store(false, Ordering::Relaxed);
                        }
                    }
                }
            }
            tokio::select! {
                () = shutdown.cancelled() => {
                    tracing::info!("Leader election task shutting down");
                    break;
                }
                () = tokio::time::sleep(renew_interval) => {}
            }
        }
    });
}

fn detect_runtime_namespace() -> String {
    std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace")
        .ok()
        .map(|namespace| namespace.trim().to_string())
        .filter(|namespace| !namespace.is_empty())
        .unwrap_or_else(|| "default".to_string())
}
