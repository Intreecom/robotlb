//! Hetzner Cloud Load Balancer API abstraction.
//!
//! This module provides a trait for interacting with the Hetzner Cloud API,
//! allowing for easier testing through mock implementations.

use async_trait::async_trait;
use hcloud::{
    apis::{
        configuration::Configuration as HcloudConfig,
        load_balancers_api::{
            AddServiceParams, AddTargetParams, AttachLoadBalancerToNetworkParams,
            ChangeAlgorithmParams, ChangeTypeOfLoadBalancerParams, DeleteLoadBalancerParams,
            DeleteServiceParams, DetachLoadBalancerFromNetworkParams, ListLoadBalancersParams,
            RemoveTargetParams, UpdateServiceParams,
        },
        networks_api::ListNetworksParams,
    },
    models::{
        AttachLoadBalancerToNetworkRequest, ChangeTypeOfLoadBalancerRequest, DeleteServiceRequest,
        DetachLoadBalancerFromNetworkRequest, LoadBalancerAddTarget, LoadBalancerAlgorithm,
        LoadBalancerService, RemoveTargetRequest, UpdateLoadBalancerService,
    },
};

use crate::error::{RobotLBError, RobotLBResult};

/// Trait for interacting with the Hetzner Cloud Load Balancer API.
///
/// This trait abstracts the Hetzner Cloud API operations, allowing for
/// mock implementations in tests.
#[async_trait]
pub trait HcloudLoadBalancerApi: Send + Sync {
    /// Update an existing service on the load balancer.
    async fn update_service(
        &self,
        load_balancer_id: i64,
        service: UpdateLoadBalancerService,
    ) -> RobotLBResult<()>;

    /// Delete a service from the load balancer.
    async fn delete_service(&self, load_balancer_id: i64, listen_port: i32) -> RobotLBResult<()>;

    /// Add a new service to the load balancer.
    async fn add_service(
        &self,
        load_balancer_id: i64,
        service: LoadBalancerService,
    ) -> RobotLBResult<()>;

    /// Remove a target from the load balancer.
    async fn remove_target(&self, load_balancer_id: i64, target_ip: String) -> RobotLBResult<()>;

    /// Add a target to the load balancer.
    async fn add_target(&self, load_balancer_id: i64, target_ip: String) -> RobotLBResult<()>;

    /// Change the load balancing algorithm.
    async fn change_algorithm(
        &self,
        load_balancer_id: i64,
        algorithm: LoadBalancerAlgorithm,
    ) -> RobotLBResult<()>;

    /// Change the load balancer type.
    async fn change_type(&self, load_balancer_id: i64, balancer_type: String) -> RobotLBResult<()>;
}

/// Live implementation of the Hetzner Cloud API.
pub struct LiveHcloudLoadBalancerApi {
    pub(crate) hcloud_config: HcloudConfig,
}

#[async_trait]
impl HcloudLoadBalancerApi for LiveHcloudLoadBalancerApi {
    async fn update_service(
        &self,
        load_balancer_id: i64,
        service: UpdateLoadBalancerService,
    ) -> RobotLBResult<()> {
        hcloud::apis::load_balancers_api::update_service(
            &self.hcloud_config,
            UpdateServiceParams {
                id: load_balancer_id,
                body: service,
            },
        )
        .await?;
        Ok(())
    }

    async fn delete_service(&self, load_balancer_id: i64, listen_port: i32) -> RobotLBResult<()> {
        hcloud::apis::load_balancers_api::delete_service(
            &self.hcloud_config,
            DeleteServiceParams {
                id: load_balancer_id,
                delete_service_request: DeleteServiceRequest { listen_port },
            },
        )
        .await?;
        Ok(())
    }

    async fn add_service(
        &self,
        load_balancer_id: i64,
        service: LoadBalancerService,
    ) -> RobotLBResult<()> {
        hcloud::apis::load_balancers_api::add_service(
            &self.hcloud_config,
            AddServiceParams {
                id: load_balancer_id,
                body: service,
            },
        )
        .await?;
        Ok(())
    }

    async fn remove_target(&self, load_balancer_id: i64, target_ip: String) -> RobotLBResult<()> {
        hcloud::apis::load_balancers_api::remove_target(
            &self.hcloud_config,
            RemoveTargetParams {
                id: load_balancer_id,
                remove_target_request: RemoveTargetRequest {
                    ip: Some(Box::new(hcloud::models::LoadBalancerTargetIp {
                        ip: target_ip,
                    })),
                    ..Default::default()
                },
            },
        )
        .await?;
        Ok(())
    }

    async fn add_target(&self, load_balancer_id: i64, target_ip: String) -> RobotLBResult<()> {
        hcloud::apis::load_balancers_api::add_target(
            &self.hcloud_config,
            AddTargetParams {
                id: load_balancer_id,
                body: LoadBalancerAddTarget {
                    ip: Some(Box::new(hcloud::models::LoadBalancerTargetIp {
                        ip: target_ip,
                    })),
                    ..Default::default()
                },
            },
        )
        .await?;
        Ok(())
    }

    async fn change_algorithm(
        &self,
        load_balancer_id: i64,
        algorithm: LoadBalancerAlgorithm,
    ) -> RobotLBResult<()> {
        hcloud::apis::load_balancers_api::change_algorithm(
            &self.hcloud_config,
            ChangeAlgorithmParams {
                id: load_balancer_id,
                body: algorithm,
            },
        )
        .await?;
        Ok(())
    }

    async fn change_type(&self, load_balancer_id: i64, balancer_type: String) -> RobotLBResult<()> {
        hcloud::apis::load_balancers_api::change_type_of_load_balancer(
            &self.hcloud_config,
            ChangeTypeOfLoadBalancerParams {
                id: load_balancer_id,
                change_type_of_load_balancer_request: ChangeTypeOfLoadBalancerRequest {
                    load_balancer_type: balancer_type,
                },
            },
        )
        .await?;
        Ok(())
    }
}

/// List load balancers from Hetzner Cloud.
pub async fn list_load_balancers(
    hcloud_config: &HcloudConfig,
    name: Option<String>,
) -> RobotLBResult<hcloud::models::ListLoadBalancersResponse> {
    Ok(hcloud::apis::load_balancers_api::list_load_balancers(
        hcloud_config,
        ListLoadBalancersParams {
            name,
            ..Default::default()
        },
    )
    .await?)
}

/// Get a specific load balancer by name.
pub async fn get_load_balancer(
    hcloud_config: &HcloudConfig,
    name: &str,
) -> RobotLBResult<Option<hcloud::models::LoadBalancer>> {
    let hcloud_balancers = list_load_balancers(hcloud_config, Some(name.to_string())).await?;
    if hcloud_balancers.load_balancers.len() > 1 {
        tracing::warn!("Found more than one balancer with name {name}, skipping");
        return Err(RobotLBError::SkipService);
    }
    Ok(hcloud_balancers.load_balancers.into_iter().next())
}

/// Create a new load balancer.
pub async fn create_load_balancer(
    hcloud_config: &HcloudConfig,
    name: &str,
    location: &str,
    balancer_type: &str,
    algorithm: LoadBalancerAlgorithm,
    public_interface: bool,
) -> RobotLBResult<hcloud::models::LoadBalancer> {
    let response = hcloud::apis::load_balancers_api::create_load_balancer(
        hcloud_config,
        hcloud::apis::load_balancers_api::CreateLoadBalancerParams {
            create_load_balancer_request: hcloud::models::CreateLoadBalancerRequest {
                algorithm: Some(Box::new(algorithm)),
                labels: None,
                load_balancer_type: balancer_type.to_string(),
                location: Some(location.to_string()),
                name: name.to_string(),
                network: None,
                network_zone: None,
                public_interface: Some(public_interface),
                services: Some(vec![]),
                targets: Some(vec![]),
            },
        },
    )
    .await;

    match response {
        Ok(created) => Ok(*created.load_balancer),
        Err(e) => {
            tracing::error!("Failed to create load balancer: {:?}", e);
            Err(RobotLBError::HCloudError(format!(
                "Failed to create load balancer: {e:?}"
            )))
        }
    }
}

/// Delete a load balancer.
pub async fn delete_load_balancer(hcloud_config: &HcloudConfig, id: i64) -> RobotLBResult<()> {
    hcloud::apis::load_balancers_api::delete_load_balancer(
        hcloud_config,
        DeleteLoadBalancerParams { id },
    )
    .await?;
    Ok(())
}

/// Delete a service from a load balancer.
pub async fn delete_service(
    hcloud_config: &HcloudConfig,
    id: i64,
    listen_port: i32,
) -> RobotLBResult<()> {
    hcloud::apis::load_balancers_api::delete_service(
        hcloud_config,
        DeleteServiceParams {
            id,
            delete_service_request: DeleteServiceRequest { listen_port },
        },
    )
    .await?;
    Ok(())
}

/// Remove a target from a load balancer.
pub async fn remove_target(
    hcloud_config: &HcloudConfig,
    id: i64,
    target_ip: hcloud::models::LoadBalancerTargetIp,
) -> RobotLBResult<()> {
    hcloud::apis::load_balancers_api::remove_target(
        hcloud_config,
        RemoveTargetParams {
            id,
            remove_target_request: RemoveTargetRequest {
                ip: Some(Box::new(target_ip)),
                ..Default::default()
            },
        },
    )
    .await?;
    Ok(())
}

/// Attach a load balancer to a network.
pub async fn attach_to_network(
    hcloud_config: &HcloudConfig,
    id: i64,
    network_id: i64,
    ip: Option<String>,
) -> RobotLBResult<()> {
    hcloud::apis::load_balancers_api::attach_load_balancer_to_network(
        hcloud_config,
        AttachLoadBalancerToNetworkParams {
            id,
            attach_load_balancer_to_network_request: AttachLoadBalancerToNetworkRequest {
                ip,
                ip_range: None,
                network: network_id,
            },
        },
    )
    .await?;
    Ok(())
}

/// Detach a load balancer from a network.
pub async fn detach_from_network(
    hcloud_config: &HcloudConfig,
    id: i64,
    network_id: i64,
) -> RobotLBResult<()> {
    hcloud::apis::load_balancers_api::detach_load_balancer_from_network(
        hcloud_config,
        DetachLoadBalancerFromNetworkParams {
            id,
            detach_load_balancer_from_network_request: DetachLoadBalancerFromNetworkRequest {
                network: network_id,
            },
        },
    )
    .await?;
    Ok(())
}

/// List networks from Hetzner Cloud.
pub async fn list_networks(
    hcloud_config: &HcloudConfig,
    name: Option<String>,
) -> RobotLBResult<hcloud::models::ListNetworksResponse> {
    Ok(hcloud::apis::networks_api::list_networks(
        hcloud_config,
        ListNetworksParams {
            name,
            ..Default::default()
        },
    )
    .await?)
}
