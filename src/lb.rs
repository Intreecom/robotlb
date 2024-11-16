use hcloud::{
    apis::{
        configuration::Configuration as HcloudConfig,
        load_balancers_api::{
            AddServiceParams, AddTargetParams, DeleteServiceParams, ListLoadBalancersParams,
            RemoveTargetParams, UpdateServiceParams,
        },
    },
    models::{
        DeleteServiceRequest, LoadBalancerAddTarget, LoadBalancerService,
        LoadBalancerServiceHealthCheck, RemoveTargetRequest, UpdateLoadBalancerService,
    },
};
use k8s_openapi::api::core::v1::Service;
use kube::ResourceExt;
use std::{collections::HashMap, str::FromStr};

use crate::{
    consts,
    error::{LBTrackerError, LBTrackerResult},
    CurrentContext,
};

#[derive(Debug)]
pub struct LBService {
    pub listen_port: i32,
    pub target_port: i32,
}

#[derive(Debug)]
pub struct LoadBalancer {
    pub name: String,
    pub services: HashMap<i32, i32>,
    pub targets: Vec<String>,

    pub check_interval: i32,
    pub timeout: i32,
    pub retries: i32,
    pub proxy_mode: bool,

    pub hcloud_config: HcloudConfig,
}

impl LoadBalancer {
    pub fn try_from_svc(svc: &Service, context: &CurrentContext) -> LBTrackerResult<Self> {
        let retries = svc
            .annotations()
            .get(consts::LB_RETRIES_LABEL_NAME)
            .map(String::as_str)
            .map(i32::from_str)
            .transpose()?
            .unwrap_or(context.config.default_lb_retries);

        let timeout = svc
            .annotations()
            .get(consts::LB_TIMEOUT_LABEL_NAME)
            .map(String::as_str)
            .map(i32::from_str)
            .transpose()?
            .unwrap_or(context.config.default_lb_timeout);

        let check_interval = svc
            .annotations()
            .get(consts::LB_CHECK_INTERVAL_LABEL_NAME)
            .map(String::as_str)
            .map(i32::from_str)
            .transpose()?
            .unwrap_or(context.config.default_lb_interval);

        let proxy_mode = svc
            .annotations()
            .get(consts::LB_PROXY_MODE_LABEL_NAME)
            .map(String::as_str)
            .map(bool::from_str)
            .transpose()?
            .unwrap_or(false);

        let Some(name) = svc.annotations().get(consts::LB_NAME_LABEL_NAME).cloned() else {
            return Err(LBTrackerError::SkipService);
        };

        Ok(Self {
            name,
            check_interval,
            timeout,
            retries,
            proxy_mode,
            services: HashMap::default(),
            targets: Vec::default(),
            hcloud_config: context.hcloud_config.clone(),
        })
    }

    pub fn add_service(&mut self, listen_port: i32, target_port: i32) {
        self.services.insert(listen_port, target_port);
    }

    pub fn add_target(&mut self, ip: &str) {
        self.targets.push(ip.to_string());
    }

    pub async fn reconcile(&self) -> LBTrackerResult<()> {
        let hcloud_balancer = self.get_hcloud_lb().await?;
        futures::try_join!(
            self.reconcile_services(&hcloud_balancer),
            self.reconcile_targets(&hcloud_balancer),
        )?;
        Ok(())
    }

    async fn reconcile_services(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> LBTrackerResult<()> {
        for service in &hcloud_balancer.services {
            if let Some(destination_port) = self.services.get(&service.listen_port) {
                if service.destination_port != *destination_port {
                    tracing::info!(
                    "Found service that listens for port {} but has wrong destination port, updating",
                    service.listen_port,
                );
                    hcloud::apis::load_balancers_api::update_service(
                        &self.hcloud_config,
                    UpdateServiceParams {
                        id: hcloud_balancer.id,
                        body: Some(UpdateLoadBalancerService {
                            listen_port: service.listen_port,
                            destination_port: Some(*destination_port),
                            health_check: Some(Box::new(
                                hcloud::models::UpdateLoadBalancerServiceHealthCheck {
                                    protocol: Some(hcloud::models::update_load_balancer_service_health_check::Protocol::Tcp),
                                    http: None,
                                    interval: Some(self.check_interval),
                                    port: Some(*destination_port),
                                    retries: Some(self.retries),
                                    timeout: Some(self.timeout),
                                },
                            )),
                            ..Default::default()
                        }),
                    },
                )
                .await?;
                }
            } else {
                tracing::info!(
                    "Deleting service that listens for port {} from load-balancer {}",
                    service.listen_port,
                    hcloud_balancer.name,
                );
                hcloud::apis::load_balancers_api::delete_service(
                    &self.hcloud_config,
                    DeleteServiceParams {
                        id: hcloud_balancer.id,
                        delete_service_request: Some(DeleteServiceRequest {
                            listen_port: service.listen_port,
                        }),
                    },
                )
                .await?;
            }
        }

        for (listen_port, destination_port) in &self.services {
            if !hcloud_balancer
                .services
                .iter()
                .any(|s| s.listen_port == *listen_port)
            {
                tracing::info!(
                    "Found missing service. Adding service that listens for port {}",
                    listen_port
                );
                hcloud::apis::load_balancers_api::add_service(
                    &self.hcloud_config,
                AddServiceParams {
                    id: hcloud_balancer.id,
                    body: Some(LoadBalancerService {
                        http: None,
                        listen_port: *listen_port,
                        destination_port: *destination_port,
                        protocol: hcloud::models::load_balancer_service::Protocol::Tcp,
                        proxyprotocol: self.proxy_mode,
                        health_check: Box::new(LoadBalancerServiceHealthCheck {
                            http: None,
                            interval: self.check_interval,
                            port: *destination_port,
                            protocol:
                                hcloud::models::load_balancer_service_health_check::Protocol::Tcp,
                            retries: self.retries,
                            timeout: self.timeout,
                        }),
                    }),
                },
            )
            .await?;
            }
        }
        Ok(())
    }

    async fn reconcile_targets(
        &self,
        hcloud_balancer: &hcloud::models::LoadBalancer,
    ) -> LBTrackerResult<()> {
        for target in &hcloud_balancer.targets {
            let Some(target_ip) = target.ip.clone() else {
                continue;
            };
            if !self.targets.contains(&target_ip.ip) {
                tracing::info!("Removing target {}", target_ip.ip);
                hcloud::apis::load_balancers_api::remove_target(
                    &self.hcloud_config,
                    RemoveTargetParams {
                        id: hcloud_balancer.id,
                        remove_target_request: Some(RemoveTargetRequest {
                            ip: Some(target_ip),
                            ..Default::default()
                        }),
                    },
                )
                .await?;
            }
        }

        for ip in &self.targets {
            if !hcloud_balancer
                .targets
                .iter()
                .any(|t| t.ip.as_ref().map(|i| i.ip.as_str()) == Some(ip))
            {
                tracing::info!("Adding target {}", ip);
                hcloud::apis::load_balancers_api::add_target(
                    &self.hcloud_config,
                    AddTargetParams {
                        id: hcloud_balancer.id,
                        body: Some(LoadBalancerAddTarget {
                            ip: Some(Box::new(hcloud::models::LoadBalancerTargetIp {
                                ip: ip.clone(),
                            })),
                            ..Default::default()
                        }),
                    },
                )
                .await?;
            }
        }
        Ok(())
    }

    pub async fn cleanup(&self) -> LBTrackerResult<()> {
        let hcloud_balancer = self.get_hcloud_lb().await?;
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
                    delete_service_request: Some(DeleteServiceRequest {
                        listen_port: service.listen_port,
                    }),
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
                        remove_target_request: Some(RemoveTargetRequest {
                            ip: Some(target_ip),
                            ..Default::default()
                        }),
                    },
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn get_hcloud_lb(&self) -> LBTrackerResult<hcloud::models::LoadBalancer> {
        let hcloud_balancers = hcloud::apis::load_balancers_api::list_load_balancers(
            &self.hcloud_config,
            ListLoadBalancersParams {
                name: Some(self.name.to_string()),
                ..Default::default()
            },
        )
        .await?;
        if hcloud_balancers.load_balancers.len() > 1 {
            tracing::warn!(
                "Found more than one balancer with name {}, skipping",
                self.name
            );
            return Err(LBTrackerError::SkipService);
        }
        let Some(balancer) = hcloud_balancers.load_balancers.into_iter().next() else {
            tracing::warn!("Balancer {} not found", self.name);
            return Err(LBTrackerError::SkipService);
        };

        Ok(balancer)
    }
}
