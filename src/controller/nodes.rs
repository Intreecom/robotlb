//! Node discovery for load balancer targets.
//!
//! This module provides functions for discovering target nodes based on
//! either pod placement (dynamic) or node selectors (static).

use std::{collections::HashSet, str::FromStr, sync::Arc};

use k8s_openapi::api::core::v1::{Node, Pod, Service};
use kube::{ResourceExt, api::ListParams};

use crate::{
    CurrentContext, consts,
    error::{RobotLBError, RobotLBResult},
    label_filter::LabelFilter,
    lb::LoadBalancer,
};

/// Determine the IP type to use for targets based on load balancer configuration.
#[must_use]
pub const fn node_ip_type(lb: &LoadBalancer) -> &'static str {
    if lb.network_name.is_none() {
        "ExternalIP"
    } else {
        "InternalIP"
    }
}

/// Extract target IPs from nodes based on the desired IP type.
#[must_use]
pub fn derive_targets(nodes: Vec<Node>, desired_ip_type: &str) -> Vec<String> {
    let mut targets = Vec::new();

    for node in nodes {
        let Some(status) = node.status else {
            continue;
        };
        let Some(addresses) = status.addresses else {
            continue;
        };
        for addr in addresses {
            if addr.type_ == desired_ip_type {
                targets.push(addr.address);
            }
        }
    }

    targets
}

/// Extract services (port mappings) from a Kubernetes service.
#[must_use]
pub fn derive_services(svc: &Service) -> Vec<(i32, i32)> {
    let mut services = Vec::new();

    for port in svc
        .spec
        .as_ref()
        .and_then(|spec| spec.ports.as_ref())
        .cloned()
        .unwrap_or_default()
    {
        let protocol = port.protocol.unwrap_or_else(|| "TCP".to_string());
        if protocol != "TCP" {
            tracing::warn!("Protocol {} is not supported. Skipping...", protocol);
            continue;
        }

        let Some(node_port) = port.node_port else {
            tracing::warn!(
                "Node port is not set for target_port {}. Skipping...",
                port.port
            );
            continue;
        };

        services.push((port.port, node_port));
    }

    services
}

/// Discover target nodes for a service.
///
/// This method uses either dynamic node selection (based on pod placement)
/// or static node selection (based on node selector annotations).
pub async fn discover_target_nodes(
    svc: &Arc<Service>,
    context: &Arc<CurrentContext>,
) -> RobotLBResult<Vec<Node>> {
    if context.config.dynamic_node_selector {
        get_nodes_dynamically(svc, context).await
    } else {
        get_nodes_by_selector(svc, context).await
    }
}

/// Apply desired state to a load balancer.
pub fn apply_desired_state(
    lb: &mut LoadBalancer,
    desired_targets: Vec<String>,
    desired_services: Vec<(i32, i32)>,
) {
    for target in desired_targets {
        lb.add_target(&target);
    }

    for (listen_port, target_port) in desired_services {
        lb.add_service(listen_port, target_port);
    }
}

/// Get nodes dynamically based on pod placement.
///
/// This method finds the nodes where the target pods are deployed
/// using the pod selector from the service.
async fn get_nodes_dynamically(
    svc: &Arc<Service>,
    context: &Arc<CurrentContext>,
) -> RobotLBResult<Vec<Node>> {
    let pod_api = kube::Api::<Pod>::namespaced(
        context.client.clone(),
        svc.namespace().as_deref().map_or_else(
            || context.client.default_namespace(),
            std::convert::identity,
        ),
    );

    let Some(pod_selector) = svc.spec.as_ref().and_then(|spec| spec.selector.clone()) else {
        return Err(RobotLBError::ServiceWithoutSelector);
    };

    let label_selector = pod_selector
        .iter()
        .map(|(key, val)| format!("{key}={val}"))
        .collect::<Vec<_>>()
        .join(",");

    let pods = pod_api
        .list(&ListParams {
            label_selector: Some(label_selector),
            ..Default::default()
        })
        .await?;

    let target_nodes = pods
        .iter()
        .filter_map(|pod| pod.spec.clone().unwrap_or_default().node_name)
        .collect::<HashSet<_>>();

    let nodes_api = kube::Api::<Node>::all(context.client.clone());
    let nodes = nodes_api
        .list(&ListParams::default())
        .await?
        .into_iter()
        .filter(|node| target_nodes.contains(&node.name_any()))
        .collect::<Vec<_>>();

    Ok(nodes)
}

/// Get nodes based on the node selector annotation.
///
/// This method finds nodes matching the label selector from the
/// service's `robotlb/node-selector` annotation.
async fn get_nodes_by_selector(
    svc: &Arc<Service>,
    context: &Arc<CurrentContext>,
) -> RobotLBResult<Vec<Node>> {
    let node_selector = svc
        .annotations()
        .get(consts::LB_NODE_SELECTOR)
        .map(String::as_str)
        .ok_or(RobotLBError::ServiceWithoutSelector)?;
    let label_filter = LabelFilter::from_str(node_selector)?;
    let nodes_api = kube::Api::<Node>::all(context.client.clone());
    let nodes = nodes_api
        .list(&ListParams::default())
        .await?
        .into_iter()
        .filter(|node| label_filter.check(node.labels()))
        .collect::<Vec<_>>();
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::{
        api::core::v1::{NodeAddress, NodeStatus, ServicePort, ServiceSpec},
        apimachinery::pkg::apis::meta::v1::ObjectMeta,
    };

    fn service_with_spec(spec: ServiceSpec) -> Service {
        Service {
            metadata: ObjectMeta {
                name: Some("svc".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: Some(spec),
            ..Default::default()
        }
    }

    fn service_spec(
        service_type: &str,
        lb_class: Option<&str>,
        ports: Vec<ServicePort>,
    ) -> ServiceSpec {
        ServiceSpec {
            type_: Some(service_type.to_string()),
            load_balancer_class: lb_class.map(str::to_string),
            ports: Some(ports),
            ..Default::default()
        }
    }

    #[test]
    fn derive_targets_picks_matching_address_type() {
        let nodes = vec![
            Node {
                status: Some(NodeStatus {
                    addresses: Some(vec![
                        NodeAddress {
                            address: "10.0.0.10".to_string(),
                            type_: "InternalIP".to_string(),
                        },
                        NodeAddress {
                            address: "203.0.113.10".to_string(),
                            type_: "ExternalIP".to_string(),
                        },
                    ]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Node {
                status: Some(NodeStatus {
                    addresses: Some(vec![NodeAddress {
                        address: "203.0.113.11".to_string(),
                        type_: "ExternalIP".to_string(),
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ];

        let targets = derive_targets(nodes, "ExternalIP");
        assert_eq!(targets, vec!["203.0.113.10", "203.0.113.11"]);
    }

    #[test]
    fn derive_services_keeps_only_tcp_node_port_pairs() {
        let svc = service_with_spec(service_spec(
            "LoadBalancer",
            None,
            vec![
                ServicePort {
                    port: 80,
                    protocol: Some("TCP".to_string()),
                    node_port: Some(30080),
                    ..Default::default()
                },
                ServicePort {
                    port: 443,
                    protocol: Some("UDP".to_string()),
                    node_port: Some(30443),
                    ..Default::default()
                },
                ServicePort {
                    port: 8080,
                    protocol: Some("TCP".to_string()),
                    node_port: None,
                    ..Default::default()
                },
            ],
        ));

        let services = derive_services(&svc);
        assert_eq!(services, vec![(80, 30080)]);
    }
}
