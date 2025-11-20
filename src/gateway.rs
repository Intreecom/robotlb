use gateway_api::apis::standard::gateways::Gateway;
use kube::ResourceExt;
use std::{collections::HashMap, str::FromStr};

use crate::{
    consts,
    error::{RobotLBError, RobotLBResult},
    lb::{LBAlgorithm, LoadBalancer},
    CurrentContext,
};

/// Struct representing a Gateway-based load balancer.
/// This wraps the existing LoadBalancer functionality but
/// is constructed from Gateway API resources instead of Services.
#[derive(Debug)]
pub struct GatewayLoadBalancer {
    pub gateway_name: String,
    pub gateway_namespace: String,
    pub lb: LoadBalancer,
}

impl GatewayLoadBalancer {
    /// Create a new `GatewayLoadBalancer` instance from a Gateway resource
    /// and the current context.
    /// This method extracts configuration from Gateway annotations and spec.
    pub fn try_from_gateway(
        gateway: &Gateway,
        context: &CurrentContext,
    ) -> RobotLBResult<Self> {
        let annotations = gateway.metadata.annotations.as_ref();

        // Parse health check configuration
        let retries = annotations
            .and_then(|a| a.get(consts::LB_RETRIES_ANN_NAME))
            .map(String::as_str)
            .map(i32::from_str)
            .transpose()?
            .unwrap_or(context.config.default_lb_retries);

        let timeout = annotations
            .and_then(|a| a.get(consts::LB_TIMEOUT_ANN_NAME))
            .map(String::as_str)
            .map(i32::from_str)
            .transpose()?
            .unwrap_or(context.config.default_lb_timeout);

        let check_interval = annotations
            .and_then(|a| a.get(consts::LB_CHECK_INTERVAL_ANN_NAME))
            .map(String::as_str)
            .map(i32::from_str)
            .transpose()?
            .unwrap_or(context.config.default_lb_interval);

        let proxy_mode = annotations
            .and_then(|a| a.get(consts::LB_PROXY_MODE_LABEL_NAME))
            .map(String::as_str)
            .map(bool::from_str)
            .transpose()?
            .unwrap_or(context.config.default_lb_proxy_mode_enabled);

        // Parse load balancer configuration
        let location = annotations
            .and_then(|a| a.get(consts::LB_LOCATION_LABEL_NAME))
            .cloned()
            .unwrap_or_else(|| context.config.default_lb_location.clone());

        let balancer_type = annotations
            .and_then(|a| a.get(consts::LB_BALANCER_TYPE_LABEL_NAME))
            .cloned()
            .unwrap_or_else(|| context.config.default_balancer_type.clone());

        let algorithm = annotations
            .and_then(|a| a.get(consts::LB_ALGORITHM_LABEL_NAME))
            .map(String::as_str)
            .or(Some(&context.config.default_lb_algorithm))
            .map(LBAlgorithm::from_str)
            .transpose()?
            .unwrap_or(LBAlgorithm::LeastConnections);

        // Network configuration
        let network_name = annotations
            .and_then(|a| a.get(consts::LB_NETWORK_LABEL_NAME))
            .or(context.config.default_network.as_ref())
            .cloned();

        let private_ip = annotations
            .and_then(|a| a.get(consts::LB_PRIVATE_IP_LABEL_NAME))
            .cloned();

        // Gateway name becomes the load balancer name
        let name = annotations
            .and_then(|a| a.get(consts::LB_NAME_LABEL_NAME))
            .cloned()
            .unwrap_or_else(|| gateway.name_any());

        let gateway_name = gateway.name_any();
        let gateway_namespace = gateway
            .namespace()
            .ok_or(RobotLBError::SkipGateway)?;

        Ok(Self {
            gateway_name: gateway_name.clone(),
            gateway_namespace,
            lb: LoadBalancer {
                name,
                private_ip,
                balancer_type,
                check_interval,
                timeout,
                retries,
                location,
                proxy_mode,
                network_name,
                algorithm: algorithm.into(),
                services: HashMap::default(),
                targets: Vec::default(),
                hcloud_config: context.hcloud_config.clone(),
            },
        })
    }

    /// Add a service (listener) to the load balancer.
    /// The service will listen on the `listen_port` and forward traffic
    /// to the `target_port` on all targets.
    pub fn add_service(&mut self, listen_port: i32, target_port: i32) {
        self.lb.add_service(listen_port, target_port);
    }

    /// Add a target to the load balancer.
    /// The target will receive traffic from the services.
    /// The target is identified by its IP address.
    pub fn add_target(&mut self, ip: &str) {
        self.lb.add_target(ip);
    }

    /// Reconcile the load balancer to match the desired configuration.
    /// This delegates to the underlying LoadBalancer reconciliation logic.
    #[tracing::instrument(skip(self), fields(gateway_name=self.gateway_name, gateway_namespace=self.gateway_namespace))]
    pub async fn reconcile(&self) -> RobotLBResult<hcloud::models::LoadBalancer> {
        self.lb.reconcile().await
    }

    /// Cleanup the load balancer.
    /// This removes all services and targets before deleting the load balancer.
    pub async fn cleanup(&self) -> RobotLBResult<()> {
        self.lb.cleanup().await
    }
}
