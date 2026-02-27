//! Service status management.
//!
//! This module provides functions for updating the status of Kubernetes
//! services, particularly for setting the load balancer ingress status.

use k8s_openapi::{api::core::v1::Service, serde_json::Value, serde_json::json};
use kube::{ResourceExt, api::Patch, api::PatchParams};

use crate::CurrentContext;
use crate::error::RobotLBResult;

/// Build ingress status entries from a Hetzner Cloud load balancer.
#[must_use]
pub fn build_ingress(
    hcloud_lb: &hcloud::models::LoadBalancer,
    enable_ipv6: bool,
    proxy_mode: bool,
) -> Vec<Value> {
    let ip_mode = if proxy_mode { "Proxy" } else { "VIP" };
    let mut ingress = vec![];

    let dns_ipv4 = hcloud_lb.public_net.ipv4.dns_ptr.clone().flatten();
    let ipv4 = hcloud_lb.public_net.ipv4.ip.clone().flatten();
    let dns_ipv6 = hcloud_lb.public_net.ipv6.dns_ptr.clone().flatten();
    let ipv6 = hcloud_lb.public_net.ipv6.ip.clone().flatten();

    if let Some(ipv4) = &ipv4 {
        ingress.push(json!({
            "ip": ipv4,
            "dns": dns_ipv4,
            "ipMode": ip_mode
        }));
    }

    if enable_ipv6 && let Some(ipv6) = &ipv6 {
        ingress.push(json!({
            "ip": ipv6,
            "dns": dns_ipv6,
            "ipMode": ip_mode
        }));
    }

    ingress
}

/// Patch the service status with the load balancer ingress.
pub async fn patch_ingress_status(
    svc: &Service,
    context: &CurrentContext,
    ingress: Vec<Value>,
) -> RobotLBResult<()> {
    if ingress.is_empty() {
        return Ok(());
    }

    let svc_api = kube::Api::<Service>::namespaced(
        context.client.clone(),
        svc.namespace()
            .unwrap_or_else(|| context.client.default_namespace().to_string())
            .as_str(),
    );

    svc_api
        .patch_status(
            svc.name_any().as_str(),
            &PatchParams::default(),
            &Patch::Merge(json!({
                "status": {
                    "loadBalancer": {
                        "ingress": ingress
                    }
                }
            })),
        )
        .await?;

    Ok(())
}
