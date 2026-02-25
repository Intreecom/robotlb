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
        LoadBalancerService, LoadBalancerServiceHealthCheck, RemoveTargetRequest,
        UpdateLoadBalancerService,
    },
};
use k8s_openapi::api::core::v1::Service;
use kube::ResourceExt;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use crate::{
    config::OperatorConfig,
    consts,
    error::{RobotLBError, RobotLBResult},
    CurrentContext,
};

#[derive(Debug)]
pub struct LBService {
    pub listen_port: i32,
    pub target_port: i32,
}

enum LBAlgorithm {
    RoundRobin,
    LeastConnections,
}

#[async_trait]
trait HcloudLoadBalancerApi: Send + Sync {
    async fn update_service(
        &self,
        load_balancer_id: i64,
        service: UpdateLoadBalancerService,
    ) -> RobotLBResult<()>;

    async fn delete_service(&self, load_balancer_id: i64, listen_port: i32) -> RobotLBResult<()>;

    async fn add_service(
        &self,
        load_balancer_id: i64,
        service: LoadBalancerService,
    ) -> RobotLBResult<()>;

    async fn remove_target(&self, load_balancer_id: i64, target_ip: String) -> RobotLBResult<()>;

    async fn add_target(&self, load_balancer_id: i64, target_ip: String) -> RobotLBResult<()>;

    async fn change_algorithm(
        &self,
        load_balancer_id: i64,
        algorithm: LoadBalancerAlgorithm,
    ) -> RobotLBResult<()>;

    async fn change_type(&self, load_balancer_id: i64, balancer_type: String) -> RobotLBResult<()>;
}

struct LiveHcloudLoadBalancerApi {
    hcloud_config: HcloudConfig,
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

/// Struct representing a load balancer
/// It holds all the necessary information to manage the load balancer
/// in Hetzner Cloud.
#[derive(Debug)]
pub struct LoadBalancer {
    pub name: String,
    pub services: HashMap<i32, i32>,
    pub targets: Vec<String>,
    pub private_ip: Option<String>,

    pub check_interval: i32,
    pub timeout: i32,
    pub retries: i32,
    pub proxy_mode: bool,

    pub location: String,
    pub balancer_type: String,
    pub algorithm: LoadBalancerAlgorithm,
    pub network_name: Option<String>,

    pub hcloud_config: HcloudConfig,
}

#[derive(Debug)]
struct ParsedLoadBalancerConfig {
    name: String,
    private_ip: Option<String>,
    balancer_type: String,
    check_interval: i32,
    timeout: i32,
    retries: i32,
    location: String,
    proxy_mode: bool,
    network_name: Option<String>,
    algorithm: LoadBalancerAlgorithm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ServiceReconcileAction {
    Update {
        listen_port: i32,
        destination_port: i32,
    },
    Delete {
        listen_port: i32,
    },
    Add {
        listen_port: i32,
        destination_port: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TargetReconcileAction {
    Remove { target_ip: String },
    Add { target_ip: String },
}

impl LoadBalancer {
    /// Create a new `LoadBalancer` instance from a Kubernetes service
    /// and the current context.
    /// This method will try to extract all the necessary information
    /// from the service annotations and the context.
    /// If some of the required information is missing, the method will
    /// try to use the default values from the context.
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
    /// The service will listen on the `listen_port` and forward the
    /// traffic to the `target_port` to all targets.
    pub fn add_service(&mut self, listen_port: i32, target_port: i32) {
        self.services.insert(listen_port, target_port);
    }

    /// Add a target to the load balancer.
    /// The target will receive the traffic from the services.
    /// The target is identified by its IP address.
    pub fn add_target(&mut self, ip: &str) {
        self.targets.push(ip.to_string());
    }

    /// Reconcile the load balancer to match the desired configuration.
    #[tracing::instrument(skip(self), fields(lb_name=self.name))]
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
                            protocol: Some(hcloud::models::update_load_balancer_service::Protocol::Tcp),
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
                if !self.service_matches_desired(service, *destination_port) {
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

    fn service_matches_desired(
        &self,
        service: &LoadBalancerService,
        destination_port: i32,
    ) -> bool {
        service.destination_port == destination_port
            && service.health_check.port == destination_port
            && service.health_check.interval == self.check_interval
            && service.health_check.retries == self.retries
            && service.health_check.timeout == self.timeout
            && service.proxyprotocol == self.proxy_mode
            && service.http.is_none()
            && service.health_check.protocol
                == hcloud::models::load_balancer_service_health_check::Protocol::Tcp
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
    /// This method will compare the desired network configuration
    /// with the current network configuration of the load balancer.
    /// If the configuration does not match, the method will update the
    /// network configuration.
    async fn reconcile_network(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> RobotLBResult<()> {
        // If the network name is not provided, and laod balancer is not attached to any network,
        // we can skip this step.
        if self.network_name.is_none() && hcloud_balancer.private_net.is_empty() {
            return Ok(());
        }

        let desired_network = self.get_network().await?.map(|network| network.id);
        // If the network name is not provided, but the load balancer is attached to a network,
        // we need to detach it from the network.
        let mut contain_desired_network = false;
        if !hcloud_balancer.private_net.is_empty() {
            for private_net in &hcloud_balancer.private_net {
                let Some(private_net_id) = private_net.network else {
                    continue;
                };
                // The load balancer is attached to a target network.
                if desired_network == Some(private_net_id) {
                    // Specific IP was provided, we need to check if the IP is the same.
                    if self.private_ip.is_some() {
                        // if IPs match, we can leave everything as it is.
                        if private_net.ip == self.private_ip {
                            contain_desired_network = true;
                            continue;
                        }
                    } else {
                        // No specific IP was provided, we can leave everything as it is.
                        contain_desired_network = true;
                        continue;
                    }
                }
                tracing::info!("Detaching balancer from network {}", private_net_id);
                hcloud::apis::load_balancers_api::detach_load_balancer_from_network(
                    &self.hcloud_config,
                    DetachLoadBalancerFromNetworkParams {
                        id: hcloud_balancer.id,
                        detach_load_balancer_from_network_request:
                            DetachLoadBalancerFromNetworkRequest {
                                network: private_net_id,
                            },
                    },
                )
                .await?;
            }
        }
        if !contain_desired_network {
            let Some(network_id) = desired_network else {
                return Ok(());
            };
            tracing::info!("Attaching balancer to network {}", network_id);
            hcloud::apis::load_balancers_api::attach_load_balancer_to_network(
                &self.hcloud_config,
                AttachLoadBalancerToNetworkParams {
                    id: hcloud_balancer.id,
                    attach_load_balancer_to_network_request: AttachLoadBalancerToNetworkRequest {
                        ip: self.private_ip.clone(),
                        ip_range: None,
                        network: network_id,
                    },
                },
            )
            .await?;
        }
        Ok(())
    }

    /// Cleanup the load balancer.
    /// This method will remove all the services and targets from the
    /// load balancer.
    pub async fn cleanup(&self) -> RobotLBResult<()> {
        let Some(hcloud_balancer) = self.get_hcloud_lb().await? else {
            return Ok(());
        };
        for service in &hcloud_balancer.services {
            tracing::info!(
                "Deleting service that listens for port {} from load-balancer {}",
                service.listen_port,
                hcloud_balancer.name,
            );
            hcloud::apis::load_balancers_api::delete_service(
                &self.hcloud_config,
                DeleteServiceParams {
                    id: hcloud_balancer.id,
                    delete_service_request: DeleteServiceRequest {
                        listen_port: service.listen_port,
                    },
                },
            )
            .await?;
        }
        for target in &hcloud_balancer.targets {
            if let Some(target_ip) = target.ip.clone() {
                tracing::info!("Removing target {}", target_ip.ip);
                hcloud::apis::load_balancers_api::remove_target(
                    &self.hcloud_config,
                    RemoveTargetParams {
                        id: hcloud_balancer.id,
                        remove_target_request: RemoveTargetRequest {
                            ip: Some(target_ip),
                            ..Default::default()
                        },
                    },
                )
                .await?;
            }
        }
        hcloud::apis::load_balancers_api::delete_load_balancer(
            &self.hcloud_config,
            DeleteLoadBalancerParams {
                id: hcloud_balancer.id,
            },
        )
        .await?;
        Ok(())
    }

    /// Get the load balancer from Hetzner Cloud.
    /// This method will try to find the load balancer with the name
    /// specified in the `LoadBalancer` struct.
    ///
    /// The method might return an error if the load balancer is not found
    /// or if there are multiple load balancers with the same name.
    async fn get_hcloud_lb(&self) -> RobotLBResult<Option<hcloud::models::LoadBalancer>> {
        let hcloud_balancers = hcloud::apis::load_balancers_api::list_load_balancers(
            &self.hcloud_config,
            ListLoadBalancersParams {
                name: Some(self.name.clone()),
                ..Default::default()
            },
        )
        .await?;
        if hcloud_balancers.load_balancers.len() > 1 {
            tracing::warn!(
                "Found more than one balancer with name {}, skipping",
                self.name
            );
            return Err(RobotLBError::SkipService);
        }
        // Here we just return the first load balancer,
        // if it exists, otherwise we return None
        Ok(hcloud_balancers.load_balancers.into_iter().next())
    }

    /// Get or create the load balancer in Hetzner Cloud.
    ///
    /// this method will try to find the load balancer with the name
    /// specified in the `LoadBalancer` struct. If the load balancer
    /// is not found, the method will create a new load balancer
    /// with the specified configuration in service's annotations.
    async fn get_or_create_hcloud_lb(&self) -> RobotLBResult<hcloud::models::LoadBalancer> {
        let hcloud_lb = self.get_hcloud_lb().await?;
        if let Some(balancer) = hcloud_lb {
            return Ok(balancer);
        }

        let response = hcloud::apis::load_balancers_api::create_load_balancer(
            &self.hcloud_config,
            hcloud::apis::load_balancers_api::CreateLoadBalancerParams {
                create_load_balancer_request: hcloud::models::CreateLoadBalancerRequest {
                    algorithm: Some(Box::new(self.algorithm.clone())),
                    labels: None,
                    load_balancer_type: self.balancer_type.clone(),
                    location: Some(self.location.clone()),
                    name: self.name.clone(),
                    network: None,
                    network_zone: None,
                    public_interface: Some(true),
                    services: Some(vec![]),
                    targets: Some(vec![]),
                },
            },
        )
        .await;
        if let Err(e) = response {
            tracing::error!("Failed to create load balancer: {:?}", e);
            return Err(RobotLBError::HCloudError(format!(
                "Failed to create load balancer: {e:?}"
            )));
        }

        Ok(*response.unwrap().load_balancer)
    }

    /// Get the network from Hetzner Cloud.
    /// This method will try to find the network with the name
    /// specified in the `LoadBalancer` struct. It returns `None` only
    /// in case the network name is not provided. If the network was not found,
    /// the error is returned.
    async fn get_network(&self) -> RobotLBResult<Option<hcloud::models::Network>> {
        let Some(network_name) = self.network_name.clone() else {
            return Ok(None);
        };
        let response = hcloud::apis::networks_api::list_networks(
            &self.hcloud_config,
            ListNetworksParams {
                name: Some(network_name.clone()),
                ..Default::default()
            },
        )
        .await?;

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

fn parse_load_balancer_config(
    svc: &Service,
    config: &OperatorConfig,
) -> RobotLBResult<ParsedLoadBalancerConfig> {
    let retries = svc
        .annotations()
        .get(consts::LB_RETRIES_ANN_NAME)
        .map(String::as_str)
        .map(i32::from_str)
        .transpose()?
        .unwrap_or(config.default_lb_retries);

    let timeout = svc
        .annotations()
        .get(consts::LB_TIMEOUT_ANN_NAME)
        .map(String::as_str)
        .map(i32::from_str)
        .transpose()?
        .unwrap_or(config.default_lb_timeout);

    let check_interval = svc
        .annotations()
        .get(consts::LB_CHECK_INTERVAL_ANN_NAME)
        .map(String::as_str)
        .map(i32::from_str)
        .transpose()?
        .unwrap_or(config.default_lb_interval);

    let proxy_mode = svc
        .annotations()
        .get(consts::LB_PROXY_MODE_LABEL_NAME)
        .map(String::as_str)
        .map(bool::from_str)
        .transpose()?
        .unwrap_or(config.default_lb_proxy_mode_enabled);

    let location = svc
        .annotations()
        .get(consts::LB_LOCATION_LABEL_NAME)
        .cloned()
        .unwrap_or_else(|| config.default_lb_location.clone());

    let balancer_type = svc
        .annotations()
        .get(consts::LB_BALANCER_TYPE_LABEL_NAME)
        .cloned()
        .unwrap_or_else(|| config.default_balancer_type.clone());

    let algorithm = svc
        .annotations()
        .get(consts::LB_ALGORITHM_LABEL_NAME)
        .map(String::as_str)
        .or(Some(&config.default_lb_algorithm))
        .map(LBAlgorithm::from_str)
        .transpose()?
        .unwrap_or(LBAlgorithm::LeastConnections);

    let network_name = svc
        .annotations()
        .get(consts::LB_NETWORK_LABEL_NAME)
        .or(config.default_network.as_ref())
        .cloned();

    let name = svc
        .annotations()
        .get(consts::LB_NAME_LABEL_NAME)
        .cloned()
        .unwrap_or_else(|| svc.name_any());

    let private_ip = svc
        .annotations()
        .get(consts::LB_PRIVATE_IP_LABEL_NAME)
        .cloned();

    Ok(ParsedLoadBalancerConfig {
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
    })
}

impl FromStr for LBAlgorithm {
    type Err = RobotLBError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "round-robin" => Ok(Self::RoundRobin),
            "least-connections" => Ok(Self::LeastConnections),
            _ => Err(RobotLBError::UnknownLBAlgorithm),
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

fn normalize_ip(ip: &str) -> String {
    ip.split('/').next().unwrap_or(ip).to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_load_balancer_config, HcloudLoadBalancerApi, LoadBalancer, ServiceReconcileAction,
        TargetReconcileAction,
    };
    use crate::{config::OperatorConfig, consts};
    use async_trait::async_trait;
    use hcloud::{
        apis::configuration::Configuration as HcloudConfig,
        models::{
            LoadBalancerAlgorithm, LoadBalancerService, LoadBalancerServiceHealthCheck,
            UpdateLoadBalancerService,
        },
    };
    use k8s_openapi::{api::core::v1::Service, apimachinery::pkg::apis::meta::v1::ObjectMeta};
    use std::{
        collections::{BTreeMap, HashMap},
        sync::Mutex,
    };
    use tracing::level_filters::LevelFilter;

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
        ) -> crate::error::RobotLBResult<()> {
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
        ) -> crate::error::RobotLBResult<()> {
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
        ) -> crate::error::RobotLBResult<()> {
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
        ) -> crate::error::RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::RemoveTarget { ip: target_ip });
            Ok(())
        }

        async fn add_target(
            &self,
            _load_balancer_id: i64,
            target_ip: String,
        ) -> crate::error::RobotLBResult<()> {
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
        ) -> crate::error::RobotLBResult<()> {
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
        ) -> crate::error::RobotLBResult<()> {
            self.calls
                .lock()
                .expect("lock should not be poisoned")
                .push(ApiCall::ChangeType { balancer_type });
            Ok(())
        }
    }

    fn base_config() -> OperatorConfig {
        OperatorConfig {
            hcloud_token: "token".to_string(),
            default_network: Some("default-net".to_string()),
            dynamic_node_selector: true,
            default_lb_retries: 3,
            default_lb_timeout: 10,
            default_lb_interval: 15,
            default_lb_location: "hel1".to_string(),
            default_balancer_type: "lb11".to_string(),
            default_lb_algorithm: "least-connections".to_string(),
            default_lb_proxy_mode_enabled: false,
            ipv6_ingress: false,
            leader_election_namespace: None,
            leader_election_lease_name: "robotlb-leader-election".to_string(),
            leader_election_lease_ttl_secs: 15,
            leader_election_renew_interval_secs: 5,
            log_level: LevelFilter::INFO,
        }
    }

    fn service_with_annotations(
        annotations: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Service {
        let annotation_map = annotations
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>();

        Service {
            metadata: ObjectMeta {
                name: Some("svc-name".to_string()),
                annotations: Some(annotation_map),
                ..Default::default()
            },
            ..Default::default()
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

    #[test]
    fn uses_defaults_when_annotations_are_missing() {
        let config = base_config();
        let svc = service_with_annotations([]);

        let parsed = parse_load_balancer_config(&svc, &config).expect("parse should succeed");

        assert_eq!(parsed.name, "svc-name");
        assert_eq!(parsed.retries, 3);
        assert_eq!(parsed.timeout, 10);
        assert_eq!(parsed.check_interval, 15);
        assert_eq!(parsed.location, "hel1");
        assert_eq!(parsed.balancer_type, "lb11");
        assert_eq!(parsed.network_name.as_deref(), Some("default-net"));
        assert!(!parsed.proxy_mode);
    }

    #[test]
    fn parses_annotations_into_load_balancer_config() {
        let mut config = base_config();
        config.default_network = None;
        let svc = service_with_annotations([
            (consts::LB_NAME_LABEL_NAME, "custom-lb"),
            (consts::LB_RETRIES_ANN_NAME, "5"),
            (consts::LB_TIMEOUT_ANN_NAME, "8"),
            (consts::LB_CHECK_INTERVAL_ANN_NAME, "20"),
            (consts::LB_PROXY_MODE_LABEL_NAME, "true"),
            (consts::LB_LOCATION_LABEL_NAME, "nbg1"),
            (consts::LB_BALANCER_TYPE_LABEL_NAME, "lb31"),
            (consts::LB_ALGORITHM_LABEL_NAME, "round-robin"),
            (consts::LB_NETWORK_LABEL_NAME, "private-net"),
            (consts::LB_PRIVATE_IP_LABEL_NAME, "10.10.0.5"),
        ]);

        let parsed = parse_load_balancer_config(&svc, &config).expect("parse should succeed");

        assert_eq!(parsed.name, "custom-lb");
        assert_eq!(parsed.retries, 5);
        assert_eq!(parsed.timeout, 8);
        assert_eq!(parsed.check_interval, 20);
        assert_eq!(parsed.location, "nbg1");
        assert_eq!(parsed.balancer_type, "lb31");
        assert_eq!(parsed.network_name.as_deref(), Some("private-net"));
        assert_eq!(parsed.private_ip.as_deref(), Some("10.10.0.5"));
        assert!(parsed.proxy_mode);
        assert_eq!(
            parsed.algorithm.r#type,
            hcloud::models::load_balancer_algorithm::Type::RoundRobin
        );
    }

    #[test]
    fn returns_error_for_invalid_algorithm_annotation() {
        let config = base_config();
        let svc = service_with_annotations([(consts::LB_ALGORITHM_LABEL_NAME, "weighted")]);

        let result = parse_load_balancer_config(&svc, &config);
        assert!(matches!(
            result,
            Err(crate::error::RobotLBError::UnknownLBAlgorithm)
        ));
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
