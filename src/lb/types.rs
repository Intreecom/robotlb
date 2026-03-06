//! Type definitions for load balancer configuration and reconciliation.
//!
//! This module contains the core types used to represent load balancer
//! configuration and the actions needed to reconcile the desired state.

use hcloud::models::{LoadBalancerAlgorithm, LoadBalancerService};

/// Represents a service exposed by the load balancer.
#[derive(Debug)]
pub(crate) struct LBService {
    /// Port the load balancer listens on.
    pub listen_port: i32,
    /// Port on the target servers to forward traffic to.
    pub target_port: i32,
}

/// Load balancing algorithm types.
pub(crate) enum LBAlgorithm {
    /// Round-robin load balancing.
    RoundRobin,
    /// Least connections load balancing.
    LeastConnections,
}

impl std::str::FromStr for LBAlgorithm {
    type Err = crate::error::RobotLBError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "round-robin" => Ok(Self::RoundRobin),
            "least-connections" => Ok(Self::LeastConnections),
            _ => Err(crate::error::RobotLBError::UnknownLBAlgorithm),
        }
    }
}

impl From<LBAlgorithm> for LoadBalancerAlgorithm {
    fn from(value: LBAlgorithm) -> Self {
        let r#type = match value {
            LBAlgorithm::RoundRobin => hcloud::models::load_balancer_algorithm::Type::RoundRobin,
            LBAlgorithm::LeastConnections => {
                hcloud::models::load_balancer_algorithm::Type::LeastConnections
            }
        };
        Self { r#type }
    }
}

/// Parsed configuration for a load balancer.
#[derive(Debug)]
pub(crate) struct ParsedLoadBalancerConfig {
    pub(crate) name: String,
    pub(crate) private_ip: Option<String>,
    pub(crate) balancer_type: String,
    pub(crate) check_interval: i32,
    pub(crate) timeout: i32,
    pub(crate) retries: i32,
    pub(crate) location: String,
    pub(crate) proxy_mode: bool,
    pub(crate) network_name: Option<String>,
    pub(crate) algorithm: LoadBalancerAlgorithm,
}

/// Actions that can be performed on a load balancer service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ServiceReconcileAction {
    /// Update an existing service configuration.
    Update {
        listen_port: i32,
        destination_port: i32,
    },
    /// Delete a service from the load balancer.
    Delete { listen_port: i32 },
    /// Add a new service to the load balancer.
    Add {
        listen_port: i32,
        destination_port: i32,
    },
}

/// Actions that can be performed on a load balancer target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetReconcileAction {
    /// Remove a target from the load balancer.
    Remove { target_ip: String },
    /// Add a target to the load balancer.
    Add { target_ip: String },
}

/// Check if a service matches the desired configuration.
#[must_use]
pub(crate) fn service_matches_desired(
    service: &LoadBalancerService,
    destination_port: i32,
    check_interval: i32,
    retries: i32,
    timeout: i32,
    proxy_mode: bool,
) -> bool {
    service.destination_port == destination_port
        && service.health_check.port == destination_port
        && service.health_check.interval == check_interval
        && service.health_check.retries == retries
        && service.health_check.timeout == timeout
        && service.proxyprotocol == proxy_mode
        && service.http.is_none()
        && service.health_check.protocol
            == hcloud::models::load_balancer_service_health_check::Protocol::Tcp
}

/// Normalize an IP address by removing any CIDR suffix.
pub(crate) fn normalize_ip(ip: &str) -> String {
    ip.split('/').next().unwrap_or(ip).to_string()
}
