//! Load balancer management for Hetzner Cloud.
//!
//! This module provides the core load balancer functionality, including:
//! - Configuration parsing from Kubernetes service annotations
//! - Reconciliation of load balancer state with Hetzner Cloud
//! - Cleanup of load balancer resources

mod api;
mod config;
mod types;

use std::collections::{HashMap, HashSet};

use hcloud::{
    apis::configuration::Configuration as HcloudConfig,
    models::{
        LoadBalancerAlgorithm, LoadBalancerService, LoadBalancerServiceHealthCheck,
        UpdateLoadBalancerService,
    },
};
use k8s_openapi::api::core::v1::Service;

use crate::{
    CurrentContext,
    error::{RobotLBError, RobotLBResult},
};

pub(crate) use api::{HcloudLoadBalancerApi, LiveHcloudLoadBalancerApi};
pub(crate) use config::parse_load_balancer_config;
use types::{ServiceReconcileAction, TargetReconcileAction, normalize_ip, service_matches_desired};

/// Struct representing a load balancer.
///
/// It holds all the necessary information to manage the load balancer
/// in Hetzner Cloud.
#[derive(Debug)]
pub struct LoadBalancer {
    /// Name of the load balancer.
    pub name: String,
    /// Services exposed by the load balancer (`listen_port` -> `target_port`).
    pub services: HashMap<i32, i32>,
    /// Target IPs for the load balancer.
    pub targets: Vec<String>,
    /// Optional private IP for the load balancer.
    pub private_ip: Option<String>,

    /// Health check interval in seconds.
    pub check_interval: i32,
    /// Health check timeout in seconds.
    pub timeout: i32,
    /// Number of health check retries.
    pub retries: i32,
    /// Whether proxy protocol is enabled.
    pub proxy_mode: bool,

    /// Hetzner Cloud location.
    pub location: String,
    /// Load balancer type (e.g., "lb11").
    pub balancer_type: String,
    /// Load balancing algorithm.
    pub algorithm: LoadBalancerAlgorithm,
    /// Optional network name for private networking.
    pub network_name: Option<String>,

    /// Hetzner Cloud API configuration.
    pub hcloud_config: HcloudConfig,
}

impl LoadBalancer {
    /// Create a new `LoadBalancer` instance from a Kubernetes service
    /// and the current context.
    ///
    /// This method will try to extract all the necessary information
    /// from the service annotations and the context.
    /// If some of the required information is missing, the method will
    /// try to use the default values from the context.
    ///
    /// # Errors
    ///
    /// Returns an error if required annotations are missing or invalid.
    pub fn try_from_svc(svc: &Service, context: &CurrentContext) -> RobotLBResult<Self> {
        let parsed = parse_load_balancer_config(svc, &context.config)?;

        Ok(Self {
            name: parsed.name,
            private_ip: parsed.private_ip,
            balancer_type: parsed.balancer_type,
            check_interval: parsed.check_interval,
            timeout: parsed.timeout,
            retries: parsed.retries,
            location: parsed.location,
            proxy_mode: parsed.proxy_mode,
            network_name: parsed.network_name,
            algorithm: parsed.algorithm,
            services: HashMap::default(),
            targets: Vec::default(),
            hcloud_config: context.hcloud_config.clone(),
        })
    }

    /// Add a service to the load balancer.
    ///
    /// The service will listen on the `listen_port` and forward the
    /// traffic to the `target_port` to all targets.
    pub fn add_service(&mut self, listen_port: i32, target_port: i32) {
        self.services.insert(listen_port, target_port);
    }

    /// Add a target to the load balancer.
    ///
    /// The target will receive the traffic from the services.
    /// The target is identified by its IP address.
    pub fn add_target(&mut self, ip: &str) {
        self.targets.push(ip.to_string());
    }

    /// Reconcile the load balancer to match the desired configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the Hetzner Cloud API call fails.
    #[tracing::instrument(skip(self), fields(lb_name = self.name))]
    pub async fn reconcile(&self) -> RobotLBResult<hcloud::models::LoadBalancer> {
        let api = LiveHcloudLoadBalancerApi {
            hcloud_config: self.hcloud_config.clone(),
        };
        let hcloud_balancer = self.get_or_create_hcloud_lb().await?;
        self.reconcile_algorithm(&hcloud_balancer, &api).await?;
        self.reconcile_lb_type(&hcloud_balancer, &api).await?;
        self.reconcile_network(&hcloud_balancer).await?;
        self.reconcile_services(&hcloud_balancer, &api).await?;
        self.reconcile_targets(&hcloud_balancer, &api).await?;
        Ok(hcloud_balancer)
    }

    /// Reconcile the services of the load balancer.
    ///
    /// This method will compare the desired configuration of the services
    /// with the current configuration of the services in the load balancer.
    /// If the configuration does not match, the method will update the service.
    async fn reconcile_services(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
        api: &impl HcloudLoadBalancerApi,
    ) -> RobotLBResult<()> {
        for action in self.plan_service_actions(hcloud_balancer) {
            match action {
                ServiceReconcileAction::Update {
                    listen_port,
                    destination_port,
                } => {
                    tracing::info!(
                        "Desired service configuration for port {} does not match current configuration. Updating ...",
                        listen_port,
                    );
                    api.update_service(
                        hcloud_balancer.id,
                        UpdateLoadBalancerService {
                            http: None,
                            protocol: Some(
                                hcloud::models::update_load_balancer_service::Protocol::Tcp,
                            ),
                            listen_port,
                            destination_port: Some(destination_port),
                            proxyprotocol: Some(self.proxy_mode),
                            health_check: Some(Box::new(
                                hcloud::models::UpdateLoadBalancerServiceHealthCheck {
                                    protocol: Some(
                                        hcloud::models::update_load_balancer_service_health_check::Protocol::Tcp,
                                    ),
                                    http: None,
                                    interval: Some(self.check_interval),
                                    port: Some(destination_port),
                                    retries: Some(self.retries),
                                    timeout: Some(self.timeout),
                                },
                            )),
                        },
                    )
                    .await?;
                }
                ServiceReconcileAction::Delete { listen_port } => {
                    tracing::info!(
                        "Deleting service that listens for port {} from load-balancer {}",
                        listen_port,
                        hcloud_balancer.name,
                    );
                    api.delete_service(hcloud_balancer.id, listen_port).await?;
                }
                ServiceReconcileAction::Add {
                    listen_port,
                    destination_port,
                } => {
                    tracing::info!(
                        "Found missing service. Adding service that listens for port {}",
                        listen_port
                    );
                    api.add_service(
                        hcloud_balancer.id,
                        LoadBalancerService {
                            http: None,
                            listen_port,
                            destination_port,
                            protocol: hcloud::models::load_balancer_service::Protocol::Tcp,
                            proxyprotocol: self.proxy_mode,
                            health_check: Box::new(LoadBalancerServiceHealthCheck {
                                http: None,
                                interval: self.check_interval,
                                port: destination_port,
                                protocol:
                                    hcloud::models::load_balancer_service_health_check::Protocol::Tcp,
                                retries: self.retries,
                                timeout: self.timeout,
                            }),
                        },
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }

    /// Reconcile the targets of the load balancer.
    ///
    /// This method will compare the desired configuration of the targets
    /// with the current configuration of the targets in the load balancer.
    /// If the configuration does not match, the method will update the target.
    async fn reconcile_targets(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
        api: &impl HcloudLoadBalancerApi,
    ) -> RobotLBResult<()> {
        for action in self.plan_target_actions(hcloud_balancer) {
            match action {
                TargetReconcileAction::Remove { target_ip } => {
                    tracing::info!("Removing target {}", target_ip);
                    api.remove_target(hcloud_balancer.id, target_ip).await?;
                }
                TargetReconcileAction::Add { target_ip } => {
                    tracing::info!("Adding target {}", target_ip);
                    api.add_target(hcloud_balancer.id, target_ip).await?;
                }
            }
        }
        Ok(())
    }

    /// Reconcile the load balancer algorithm.
    ///
    /// This method will compare the desired algorithm configuration
    /// and update it if it does not match the current configuration.
    async fn reconcile_algorithm(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
        api: &impl HcloudLoadBalancerApi,
    ) -> RobotLBResult<()> {
        if let Some(algorithm) = self.plan_algorithm_action(hcloud_balancer) {
            tracing::info!(
                "Changing load balancer algorithm from {:?} to {:?}",
                hcloud_balancer.algorithm,
                algorithm
            );
            api.change_algorithm(hcloud_balancer.id, algorithm).await?;
        }
        Ok(())
    }

    /// Reconcile the load balancer type.
    async fn reconcile_lb_type(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
        api: &impl HcloudLoadBalancerApi,
    ) -> RobotLBResult<()> {
        if let Some(balancer_type) = self.plan_lb_type_action(hcloud_balancer) {
            tracing::info!(
                "Changing load balancer type from {} to {}",
                hcloud_balancer.load_balancer_type.name,
                balancer_type
            );
            api.change_type(hcloud_balancer.id, balancer_type).await?;
        }
        Ok(())
    }

    fn plan_service_actions(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> Vec<ServiceReconcileAction> {
        let mut actions = Vec::new();

        for service in &hcloud_balancer.services {
            if let Some(destination_port) = self.services.get(&service.listen_port) {
                if !service_matches_desired(
                    service,
                    *destination_port,
                    self.check_interval,
                    self.retries,
                    self.timeout,
                    self.proxy_mode,
                ) {
                    actions.push(ServiceReconcileAction::Update {
                        listen_port: service.listen_port,
                        destination_port: *destination_port,
                    });
                }
            } else {
                actions.push(ServiceReconcileAction::Delete {
                    listen_port: service.listen_port,
                });
            }
        }

        let mut missing_services = self
            .services
            .iter()
            .filter(|(listen_port, _)| {
                !hcloud_balancer
                    .services
                    .iter()
                    .any(|service| service.listen_port == **listen_port)
            })
            .map(
                |(listen_port, destination_port)| ServiceReconcileAction::Add {
                    listen_port: *listen_port,
                    destination_port: *destination_port,
                },
            )
            .collect::<Vec<_>>();

        missing_services.sort_by_key(|action| match action {
            ServiceReconcileAction::Add { listen_port, .. } => *listen_port,
            _ => 0,
        });

        actions.extend(missing_services);
        actions
    }

    fn plan_target_actions(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> Vec<TargetReconcileAction> {
        let mut actions = Vec::new();

        let desired_targets: HashSet<String> =
            self.targets.iter().map(|ip| normalize_ip(ip)).collect();
        let current_targets: HashSet<String> = hcloud_balancer
            .targets
            .iter()
            .filter_map(|target| {
                target
                    .ip
                    .as_ref()
                    .map(|target_ip| normalize_ip(&target_ip.ip))
            })
            .collect();

        for target in &hcloud_balancer.targets {
            let Some(target_ip) = target.ip.clone() else {
                continue;
            };
            let normalized_target_ip = normalize_ip(&target_ip.ip);
            if !desired_targets.contains(&normalized_target_ip) {
                actions.push(TargetReconcileAction::Remove {
                    target_ip: target_ip.ip,
                });
            }
        }

        for ip in desired_targets {
            if !current_targets.contains(&ip) {
                actions.push(TargetReconcileAction::Add { target_ip: ip });
            }
        }

        actions
    }

    fn plan_algorithm_action(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> Option<LoadBalancerAlgorithm> {
        if *hcloud_balancer.algorithm == self.algorithm {
            None
        } else {
            Some(self.algorithm.clone())
        }
    }

    fn plan_lb_type_action(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> Option<String> {
        if hcloud_balancer.load_balancer_type.name == self.balancer_type {
            None
        } else {
            Some(self.balancer_type.clone())
        }
    }

    /// Reconcile the network of the load balancer.
    ///
    /// This method will compare the desired network configuration
    /// with the current network configuration of the load balancer.
    /// If the configuration does not match, the method will update the
    /// network configuration.
    async fn reconcile_network(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> RobotLBResult<()> {
        if self.network_name.is_none() && hcloud_balancer.private_net.is_empty() {
            return Ok(());
        }

        let desired_network = self.get_network().await?.map(|network| network.id);
        
        let is_attached = self.detach_unwanted_networks(hcloud_balancer, desired_network).await?;
        
        if !is_attached
            && let Some(network_id) = desired_network
        {
            tracing::info!("Attaching balancer to network {}", network_id);
            api::attach_to_network(
                &self.hcloud_config,
                hcloud_balancer.id,
                network_id,
                self.private_ip.clone(),
            )
            .await?;
        }
        Ok(())
    }

    async fn detach_unwanted_networks(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
        desired_network: Option<i64>,
    ) -> RobotLBResult<bool> {
        let mut is_attached = false;
        
        for private_net in &hcloud_balancer.private_net {
            let Some(private_net_id) = private_net.network else {
                continue;
            };
            
            if desired_network == Some(private_net_id) 
                && private_net.ip.as_ref().is_some_and(|ip| self.matches_desired_ip(ip)) {
                is_attached = true;
                continue;
            }
            
            tracing::info!("Detaching balancer from network {}", private_net_id);
            api::detach_from_network(&self.hcloud_config, hcloud_balancer.id, private_net_id)
                .await?;
        }
        
        Ok(is_attached)
    }

    fn matches_desired_ip(&self, current_ip: &str) -> bool {
        self.private_ip.as_ref().is_none_or(|desired_ip| desired_ip == current_ip)
    }

    /// Cleanup the load balancer.
    ///
    /// This method will remove all the services and targets from the
    /// load balancer.
    ///
    /// # Errors
    ///
    /// Returns an error if the Hetzner Cloud API call fails.
    pub async fn cleanup(&self) -> RobotLBResult<()> {
        let Some(hcloud_balancer) = api::get_load_balancer(&self.hcloud_config, &self.name).await?
        else {
            return Ok(());
        };
        for service in &hcloud_balancer.services {
            tracing::info!(
                "Deleting service that listens for port {} from load-balancer {}",
                service.listen_port,
                hcloud_balancer.name,
            );
            api::delete_service(&self.hcloud_config, hcloud_balancer.id, service.listen_port)
                .await?;
        }
        for target in &hcloud_balancer.targets {
            if let Some(target_ip) = target.ip.clone() {
                tracing::info!("Removing target {}", target_ip.ip);
                api::remove_target(&self.hcloud_config, hcloud_balancer.id, *target_ip).await?;
            }
        }
        api::delete_load_balancer(&self.hcloud_config, hcloud_balancer.id).await?;
        Ok(())
    }

    /// Get the load balancer from Hetzner Cloud.
    ///
    /// This method will try to find the load balancer with the name
    /// specified in the `LoadBalancer` struct.
    ///
    /// The method might return an error if the load balancer is not found
    /// or if there are multiple load balancers with the same name.
    async fn get_hcloud_lb(&self) -> RobotLBResult<Option<hcloud::models::LoadBalancer>> {
        api::get_load_balancer(&self.hcloud_config, &self.name).await
    }

    /// Get or create the load balancer in Hetzner Cloud.
    ///
    /// This method will try to find the load balancer with the name
    /// specified in the `LoadBalancer` struct. If the load balancer
    /// is not found, the method will create a new load balancer
    /// with the specified configuration in service's annotations.
    async fn get_or_create_hcloud_lb(&self) -> RobotLBResult<hcloud::models::LoadBalancer> {
        let hcloud_lb = self.get_hcloud_lb().await?;
        if let Some(balancer) = hcloud_lb {
            return Ok(balancer);
        }

        api::create_load_balancer(
            &self.hcloud_config,
            &self.name,
            &self.location,
            &self.balancer_type,
            self.algorithm.clone(),
        )
        .await
    }

    /// Get the network from Hetzner Cloud.
    ///
    /// This method will try to find the network with the name
    /// specified in the `LoadBalancer` struct. It returns `None` only
    /// in case the network name is not provided. If the network was not found,
    /// the error is returned.
    async fn get_network(&self) -> RobotLBResult<Option<hcloud::models::Network>> {
        let Some(network_name) = self.network_name.clone() else {
            return Ok(None);
        };
        let response = api::list_networks(&self.hcloud_config, Some(network_name.clone())).await?;

        if response.networks.len() > 1 {
            tracing::warn!(
                "Found more than one network with name {}, skipping",
                network_name
            );
            return Err(RobotLBError::HCloudError(format!(
                "Found more than one network with name {network_name}"
            )));
        }
        if response.networks.is_empty() {
            tracing::warn!("Network with name {} not found", network_name);
            return Err(RobotLBError::HCloudError(format!(
                "Network with name {network_name} not found"
            )));
        }

        Ok(response.networks.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hcloud::apis::configuration::Configuration as HcloudConfig;
    use std::sync::Mutex;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ApiCall {
        UpdateService {
            listen_port: i32,
            destination_port: i32,
        },
        DeleteService {
            listen_port: i32,
        },
        AddService {
            listen_port: i32,
            destination_port: i32,
        },
        RemoveTarget {
            ip: String,
        },
        AddTarget {
            ip: String,
        },
        ChangeAlgorithm,
        ChangeType {
            balancer_type: String,
        },
    }

    #[derive(Default)]
    struct MockHcloudApi {
        calls: Mutex<Vec<ApiCall>>,
    }

    impl MockHcloudApi {
        fn calls(&self) -> Vec<ApiCall> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .clone()
        }
    }

    #[async_trait]
    impl HcloudLoadBalancerApi for MockHcloudApi {
        async fn update_service(
            &self,
            _load_balancer_id: i64,
            service: UpdateLoadBalancerService,
        ) -> RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::UpdateService {
                    listen_port: service.listen_port,
                    destination_port: service
                        .destination_port
                        .expect("destination port should be present"),
                });
            Ok(())
        }

        async fn delete_service(
            &self,
            _load_balancer_id: i64,
            listen_port: i32,
        ) -> RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::DeleteService { listen_port });
            Ok(())
        }

        async fn add_service(
            &self,
            _load_balancer_id: i64,
            service: LoadBalancerService,
        ) -> RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::AddService {
                    listen_port: service.listen_port,
                    destination_port: service.destination_port,
                });
            Ok(())
        }

        async fn remove_target(
            &self,
            _load_balancer_id: i64,
            target_ip: String,
        ) -> RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::RemoveTarget { ip: target_ip });
            Ok(())
        }

        async fn add_target(&self, _load_balancer_id: i64, target_ip: String) -> RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::AddTarget { ip: target_ip });
            Ok(())
        }

        async fn change_algorithm(
            &self,
            _load_balancer_id: i64,
            _algorithm: LoadBalancerAlgorithm,
        ) -> RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::ChangeAlgorithm);
            Ok(())
        }

        async fn change_type(
            &self,
            _load_balancer_id: i64,
            balancer_type: String,
        ) -> RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::ChangeType { balancer_type });
            Ok(())
        }
    }

    fn test_load_balancer() -> LoadBalancer {
        LoadBalancer {
            name: "svc-name".to_string(),
            services: HashMap::new(),
            targets: vec![],
            private_ip: None,
            check_interval: 15,
            timeout: 10,
            retries: 3,
            proxy_mode: false,
            location: "hel1".to_string(),
            balancer_type: "lb11".to_string(),
            algorithm: LoadBalancerAlgorithm {
                r#type: hcloud::models::load_balancer_algorithm::Type::LeastConnections,
            },
            network_name: None,
            hcloud_config: HcloudConfig::new(),
        }
    }

    fn existing_service(listen_port: i32, destination_port: i32) -> LoadBalancerService {
        LoadBalancerService {
            http: None,
            listen_port,
            destination_port,
            protocol: hcloud::models::load_balancer_service::Protocol::Tcp,
            proxyprotocol: false,
            health_check: Box::new(LoadBalancerServiceHealthCheck {
                http: None,
                interval: 5,
                port: destination_port,
                protocol: hcloud::models::load_balancer_service_health_check::Protocol::Tcp,
                retries: 1,
                timeout: 2,
            }),
        }
    }

    fn remote_balancer_with_services(
        services: Vec<LoadBalancerService>,
        targets: Vec<&str>,
    ) -> hcloud::models::LoadBalancer {
        hcloud::models::LoadBalancer {
            id: 42,
            name: "svc-name".to_string(),
            services,
            targets: targets
                .into_iter()
                .map(|ip| hcloud::models::LoadBalancerTarget {
                    ip: Some(Box::new(hcloud::models::LoadBalancerTargetIp {
                        ip: ip.to_string(),
                    })),
                    ..Default::default()
                })
                .collect(),
            algorithm: Box::new(LoadBalancerAlgorithm {
                r#type: hcloud::models::load_balancer_algorithm::Type::RoundRobin,
            }),
            load_balancer_type: Box::new(hcloud::models::LoadBalancerType {
                id: 1,
                name: "lb11".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn reconcile_services_uses_mock_api_for_drift_and_missing_ports() {
        let mut lb = test_load_balancer();
        lb.services.insert(80, 30080);
        lb.services.insert(443, 30443);

        let remote = remote_balancer_with_services(
            vec![existing_service(80, 30081), existing_service(8080, 38080)],
            vec![],
        );
        let api = MockHcloudApi::default();

        lb.reconcile_services(&remote, &api)
            .await
            .expect("service reconcile should succeed");

        assert_eq!(
            api.calls(),
            vec![
                ApiCall::UpdateService {
                    listen_port: 80,
                    destination_port: 30080,
                },
                ApiCall::DeleteService { listen_port: 8080 },
                ApiCall::AddService {
                    listen_port: 443,
                    destination_port: 30443,
                }
            ]
        );
    }

    #[tokio::test]
    async fn reconcile_targets_uses_mock_api_for_target_diff() {
        let mut lb = test_load_balancer();
        lb.targets = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];

        let remote = remote_balancer_with_services(vec![], vec!["10.0.0.2", "10.0.0.3"]);
        let api = MockHcloudApi::default();

        lb.reconcile_targets(&remote, &api)
            .await
            .expect("target reconcile should succeed");

        assert_eq!(
            api.calls(),
            vec![
                ApiCall::RemoveTarget {
                    ip: "10.0.0.3".to_string(),
                },
                ApiCall::AddTarget {
                    ip: "10.0.0.1".to_string(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn reconcile_algorithm_and_type_uses_mock_api_when_values_change() {
        let mut lb = test_load_balancer();
        lb.balancer_type = "lb21".to_string();

        let remote = remote_balancer_with_services(vec![], vec![]);
        let api = MockHcloudApi::default();

        lb.reconcile_algorithm(&remote, &api)
            .await
            .expect("algorithm reconcile should succeed");
        lb.reconcile_lb_type(&remote, &api)
            .await
            .expect("type reconcile should succeed");

        assert_eq!(
            api.calls(),
            vec![
                ApiCall::ChangeAlgorithm,
                ApiCall::ChangeType {
                    balancer_type: "lb21".to_string(),
                }
            ]
        );
    }

    #[test]
    fn plan_service_actions_builds_expected_diff() {
        let mut lb = test_load_balancer();
        lb.services.insert(80, 30080);
        lb.services.insert(443, 30443);

        let remote = remote_balancer_with_services(
            vec![existing_service(80, 30081), existing_service(8080, 38080)],
            vec![],
        );

        let plan = lb.plan_service_actions(&remote);

        assert_eq!(
            plan,
            vec![
                ServiceReconcileAction::Update {
                    listen_port: 80,
                    destination_port: 30080,
                },
                ServiceReconcileAction::Delete { listen_port: 8080 },
                ServiceReconcileAction::Add {
                    listen_port: 443,
                    destination_port: 30443,
                },
            ]
        );
    }

    #[test]
    fn plan_target_actions_builds_expected_diff() {
        let mut lb = test_load_balancer();
        lb.targets = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];

        let remote = remote_balancer_with_services(vec![], vec!["10.0.0.2", "10.0.0.3"]);

        let plan = lb.plan_target_actions(&remote);

        assert_eq!(
            plan,
            vec![
                TargetReconcileAction::Remove {
                    target_ip: "10.0.0.3".to_string(),
                },
                TargetReconcileAction::Add {
                    target_ip: "10.0.0.1".to_string(),
                },
            ]
        );
    }
}
