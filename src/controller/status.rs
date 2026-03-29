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
    public_interface: bool,
) -> Vec<Value> {
    let ip_mode = if proxy_mode { "Proxy" } else { "VIP" };
    let mut ingress = vec![];

    if !public_interface {
        for private_net in &hcloud_lb.private_net {
            if let Some(ip) = &private_net.ip {
                ingress.push(json!({
                    "ip": ip,
                    "ipMode": ip_mode
                }));
            }
        }
        return ingress;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_load_balancer() -> hcloud::models::LoadBalancer {
        hcloud::models::LoadBalancer {
            public_net: Box::new(hcloud::models::LoadBalancerPublicNet {
                ipv4: Box::new(hcloud::models::LoadBalancerPublicNetIpv4 {
                    ip: Some(Some("203.0.113.10".to_string())),
                    dns_ptr: Some(Some("public.example.com".to_string())),
                }),
                ipv6: Box::new(hcloud::models::LoadBalancerPublicNetIpv6 {
                    ip: Some(Some("2001:db8::10".to_string())),
                    dns_ptr: Some(Some("public-v6.example.com".to_string())),
                }),
                ..Default::default()
            }),
            private_net: vec![hcloud::models::LoadBalancerPrivateNet {
                ip: Some("10.10.0.5".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn build_ingress_uses_public_addresses_when_enabled() {
        let ingress = build_ingress(&test_load_balancer(), true, false, true);

        assert_eq!(
            ingress,
            vec![
                json!({
                    "ip": "203.0.113.10",
                    "dns": "public.example.com",
                    "ipMode": "VIP"
                }),
                json!({
                    "ip": "2001:db8::10",
                    "dns": "public-v6.example.com",
                    "ipMode": "VIP"
                })
            ]
        );
    }

    #[test]
    fn build_ingress_uses_private_address_when_public_interface_is_disabled() {
        let ingress = build_ingress(&test_load_balancer(), true, false, false);

        assert_eq!(
            ingress,
            vec![json!({
                "ip": "10.10.0.5",
                "ipMode": "VIP"
            })]
        );
    }
}
