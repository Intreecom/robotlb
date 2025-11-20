use gateway_api::apis::{experimental::tcproutes::TCPRoute, standard::httproutes::HTTPRoute};
use k8s_openapi::api::core::v1::Service;
use kube::{Api, ResourceExt};
use std::collections::HashMap;

use crate::{
    error::{RobotLBError, RobotLBResult},
    CurrentContext,
};

/// Information about a backend service extracted from a route.
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub service_name: String,
    pub service_namespace: String,
    pub port: Option<i32>,
}

/// Information extracted from routes for a specific Gateway.
#[derive(Debug)]
pub struct RouteInfo {
    pub backend_services: Vec<BackendInfo>,
    pub port_mappings: HashMap<i32, i32>, // listener port -> backend port
}

/// Extract backend services from an HTTPRoute resource.
/// Returns a list of BackendInfo containing service names, namespaces, and ports.
pub fn extract_http_route_backends(
    route: &HTTPRoute,
    gateway_name: &str,
    gateway_namespace: &str,
) -> RobotLBResult<Vec<BackendInfo>> {
    let route_namespace = route
        .namespace()
        .unwrap_or_else(|| gateway_namespace.to_string());

    // Check if this route references our Gateway
    let parent_refs = route
        .spec
        .parent_refs
        .as_ref()
        .ok_or(RobotLBError::RouteWithoutParentRefs)?;

    let references_our_gateway = parent_refs.iter().any(|parent| {
        let parent_name = parent.name.as_str();
        let parent_namespace = parent
            .namespace
            .as_ref()
            .map(String::as_str)
            .unwrap_or(&route_namespace);

        parent_name == gateway_name && parent_namespace == gateway_namespace
    });

    if !references_our_gateway {
        return Ok(vec![]);
    }

    let mut backends = Vec::new();

    // Extract backends from all rules
    if let Some(rules) = &route.spec.rules {
        for rule in rules {
            if let Some(backend_refs) = &rule.backend_refs {
                for backend_ref in backend_refs {
                    let backend_name = backend_ref.name.clone();
                    let backend_namespace = backend_ref
                        .namespace
                        .as_ref()
                        .map(String::as_str)
                        .unwrap_or(&route_namespace)
                        .to_string();

                    let port = backend_ref.port.map(|p| p as i32);

                    backends.push(BackendInfo {
                        service_name: backend_name,
                        service_namespace: backend_namespace,
                        port,
                    });
                }
            }
        }
    }

    Ok(backends)
}

/// Extract backend services from a TCPRoute resource.
/// Returns a list of BackendInfo containing service names, namespaces, and ports.
pub fn extract_tcp_route_backends(
    route: &TCPRoute,
    gateway_name: &str,
    gateway_namespace: &str,
) -> RobotLBResult<Vec<BackendInfo>> {
    let route_namespace = route
        .namespace()
        .unwrap_or_else(|| gateway_namespace.to_string());

    // Check if this route references our Gateway
    let parent_refs = route
        .spec
        .parent_refs
        .as_ref()
        .ok_or(RobotLBError::RouteWithoutParentRefs)?;

    let references_our_gateway = parent_refs.iter().any(|parent| {
        let parent_name = parent.name.as_str();
        let parent_namespace = parent
            .namespace
            .as_ref()
            .map(String::as_str)
            .unwrap_or(&route_namespace);

        parent_name == gateway_name && parent_namespace == gateway_namespace
    });

    if !references_our_gateway {
        return Ok(vec![]);
    }

    let mut backends = Vec::new();

    // Extract backends from all rules
    for rule in &route.spec.rules {
        for backend_ref in &rule.backend_refs {
            let backend_name = backend_ref.name.clone();
            let backend_namespace = backend_ref
                .namespace
                .as_ref()
                .map(String::as_str)
                .unwrap_or(&route_namespace)
                .to_string();

            let port = backend_ref.port.map(|p| p as i32);

            backends.push(BackendInfo {
                service_name: backend_name,
                service_namespace: backend_namespace,
                port,
            });
        }
    }

    Ok(backends)
}

/// Get all Services referenced by route backends.
/// This fetches the actual Service resources from Kubernetes.
pub async fn get_backend_services(
    backends: &[BackendInfo],
    context: &CurrentContext,
) -> RobotLBResult<Vec<Service>> {
    let mut services = Vec::new();
    let mut seen_services = Vec::new();

    for backend in backends {
        let service_key = format!("{}/{}", backend.service_namespace, backend.service_name);

        // Skip if we've already fetched this service
        if seen_services.contains(&service_key) {
            continue;
        }
        seen_services.push(service_key);

        let service_api =
            Api::<Service>::namespaced(context.client.clone(), &backend.service_namespace);

        match service_api.get(&backend.service_name).await {
            Ok(service) => {
                services.push(service);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to get backend service {}/{}: {}",
                    backend.service_namespace,
                    backend.service_name,
                    e
                );
            }
        }
    }

    Ok(services)
}

/// Determine port mappings for the load balancer based on Gateway listeners and routes.
/// Returns a HashMap of listener_port -> target_port mappings.
pub fn determine_port_mappings(
    listener_ports: &[i32],
    backends: &[BackendInfo],
) -> HashMap<i32, i32> {
    let mut port_mappings = HashMap::new();

    // For each listener port, try to find a matching backend port
    for listener_port in listener_ports {
        // Try to find a backend that specifies a port
        let backend_port = backends
            .iter()
            .find_map(|b| b.port)
            .unwrap_or(*listener_port);

        port_mappings.insert(*listener_port, backend_port);
    }

    port_mappings
}
