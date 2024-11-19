#![warn(
    // Base lints.
    clippy::all,
    // Some pedantic lints.
    clippy::pedantic,
    // New lints which are cool.
    clippy::nursery,
)]
#![
    allow(
        // I don't care about this.
        clippy::module_name_repetitions,
        // Yo, the hell you should put
        // it in docs, if signature is clear as sky.
        clippy::missing_errors_doc
    )
]

use clap::Parser;
use config::OperatorConfig;
use error::{LBTrackerError, LBTrackerResult};
use futures::StreamExt;
use hcloud::apis::configuration::Configuration as HCloudConfig;
use k8s_openapi::{
    api::core::v1::{Node, Pod, Service},
    serde_json::json,
};
use kube::{
    api::{ListParams, PatchParams},
    runtime::{controller::Action, watcher, Controller},
    Resource, ResourceExt,
};
use label_filter::LabelFilter;
use lb::LoadBalancer;
use std::{collections::HashSet, str::FromStr, sync::Arc, time::Duration};

pub mod config;
pub mod consts;
pub mod error;
pub mod finalizers;
pub mod label_filter;
pub mod lb;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> LBTrackerResult<()> {
    dotenvy::dotenv().ok();
    let operator_config = config::OperatorConfig::parse();
    tracing_subscriber::fmt()
        .with_max_level(operator_config.log_level)
        .init();

    let mut hcloud_conf = HCloudConfig::new();
    hcloud_conf.bearer_access_token = Some(operator_config.hcloud_token.clone());

    tracing::info!("Starting robotlb operator v{}", env!("CARGO_PKG_VERSION"));
    let kube_client = kube::Client::try_default().await?;
    tracing::info!("Kube client is connected");
    watcher::Config::default();
    let context = Arc::new(CurrentContext::new(
        kube_client.clone(),
        operator_config.clone(),
        hcloud_conf,
    ));
    tracing::info!("Starting the controller");
    Controller::new(
        kube::Api::<Service>::all(kube_client),
        watcher::Config::default(),
    )
    .run(reconcile_service, on_error, context)
    .for_each(|reconcilation_result| async move {
        match reconcilation_result {
            Ok((service, _action)) => {
                tracing::info!("Reconcilation of a service {} was successful", service.name);
            }
            Err(err) => match err {
                // During reconcilation process,
                // the controller has decided to skip the service.
                kube::runtime::controller::Error::ReconcilerFailed(
                    LBTrackerError::SkipService,
                    _,
                ) => {}
                _ => {
                    tracing::error!("Error reconciling service: {:#?}", err);
                }
            },
        }
    })
    .await;
    Ok(())
}

#[derive(Clone)]
pub struct CurrentContext {
    pub client: kube::Client,
    pub config: OperatorConfig,
    pub hcloud_config: HCloudConfig,
}
impl CurrentContext {
    #[must_use]
    pub const fn new(
        client: kube::Client,
        config: OperatorConfig,
        hcloud_config: HCloudConfig,
    ) -> Self {
        Self {
            client,
            config,
            hcloud_config,
        }
    }
}

/// Reconcile the service.
/// This function is called by the controller for each service.
/// It will create or update the load balancer based on the service.
/// If the service is being deleted, it will clean up the resources.
#[tracing::instrument(skip(svc,context), fields(service=svc.name_any()))]
pub async fn reconcile_service(
    svc: Arc<Service>,
    context: Arc<CurrentContext>,
) -> LBTrackerResult<Action> {
    let svc_type = svc
        .spec
        .as_ref()
        .and_then(|s| s.type_.as_ref())
        .map(String::as_str)
        .unwrap_or("ClusterIP");
    if svc_type != "LoadBalancer" {
        tracing::debug!("Service type is not LoadBalancer. Skipping...");
        return Err(LBTrackerError::SkipService);
    }

    tracing::info!("Starting service reconcilation");

    let lb = LoadBalancer::try_from_svc(&svc, &context)?;

    // If the service is being deleted, we need to clean up the resources.
    if svc.meta().deletion_timestamp.is_some() {
        tracing::info!("Service deletion detected. Cleaning up resources.");
        lb.cleanup().await?;
        finalizers::remove(context.client.clone(), &svc).await?;
        return Ok(Action::await_change());
    }

    // Add finalizer if it's not there yet.
    if !finalizers::check(&svc) {
        finalizers::add(context.client.clone(), &svc).await?;
    }

    // Based on the service type, we will reconcile the load balancer.
    reconcile_load_balancer(lb, svc.clone(), context).await
}

/// Method to get nodes dynamically based on the pods.
/// This method will find the nodes where the target pods are deployed.
/// It will use the pod selector to find the pods and then get the nodes.
async fn get_nodes_dynamically(
    svc: &Arc<Service>,
    context: &Arc<CurrentContext>,
) -> LBTrackerResult<Vec<Node>> {
    let pod_api = kube::Api::<Pod>::namespaced(
        context.client.clone(),
        svc.namespace()
            .as_ref()
            .map(String::as_str)
            .unwrap_or_else(|| context.client.default_namespace()),
    );

    let Some(pod_selector) = svc.spec.as_ref().and_then(|spec| spec.selector.clone()) else {
        return Err(LBTrackerError::ServiceWithoutSelector);
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
        .map(|pod| pod.spec.clone().unwrap_or_default().node_name)
        .flatten()
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

/// Get nodes based on the node selector.
/// This method will find the nodes based on the node selector
/// from the service annotations.
async fn get_nodes_by_selector(
    svc: &Arc<Service>,
    context: &Arc<CurrentContext>,
) -> LBTrackerResult<Vec<Node>> {
    let node_selector = svc
        .annotations()
        .get(consts::LB_NODE_SELECTOR)
        .map(String::as_str)
        .ok_or(LBTrackerError::ServiceWithoutSelector)?;
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

/// Reconcile the `LoadBalancer` type of service.
/// This function will find the nodes based on the node selector
/// and create or update the load balancer.
pub async fn reconcile_load_balancer(
    mut lb: LoadBalancer,
    svc: Arc<Service>,
    context: Arc<CurrentContext>,
) -> LBTrackerResult<Action> {
    let mut node_ip_type = "InternalIP";
    if lb.network_name.is_none() {
        node_ip_type = "ExternalIP";
    }

    let nodes = if context.config.dynamic_node_selector {
        get_nodes_dynamically(&svc, &context).await?
    } else {
        get_nodes_by_selector(&svc, &context).await?
    };

    for node in nodes {
        let Some(status) = node.status else {
            continue;
        };
        let Some(addresses) = status.addresses else {
            continue;
        };
        for addr in addresses {
            if addr.type_ == node_ip_type {
                lb.add_target(&addr.address);
            }
        }
    }

    for port in svc
        .spec
        .clone()
        .unwrap_or_default()
        .ports
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
        lb.add_service(port.port, node_port);
    }

    let svc_api = kube::Api::<Service>::namespaced(
        context.client.clone(),
        svc.namespace()
            .unwrap_or_else(|| context.client.default_namespace().to_string())
            .as_str(),
    );

    let hcloud_lb = lb.reconcile().await?;

    let mut ingress = vec![];

    let dns_ipv4 = hcloud_lb.public_net.ipv4.dns_ptr.flatten();
    let ipv4 = hcloud_lb.public_net.ipv4.ip.flatten();
    let dns_ipv6 = hcloud_lb.public_net.ipv6.dns_ptr.flatten();
    let ipv6 = hcloud_lb.public_net.ipv6.ip.flatten();
    if let Some(ipv4) = &ipv4 {
        ingress.push(json!({
            "ip": ipv4,
            "dns": dns_ipv4,
            "ip_mode": "VIP"
        }))
    }
    if context.config.ipv6_ingress {
        if let Some(ipv6) = &ipv6 {
            ingress.push(json!({
                "ip": ipv6,
                "dns": dns_ipv6,
                "ip_mode": "VIP"
            }))
        }
    }

    if !ingress.is_empty() {
        svc_api
            .patch_status(
                svc.name_any().as_str(),
                &PatchParams::default(),
                &kube::api::Patch::Merge(json!({
                    "status" :{
                        "loadBalancer": {
                            "ingress": ingress
                        }
                    }
                })),
            )
            .await?;
    }

    Ok(Action::requeue(Duration::from_secs(30)))
}

/// Handle the error during reconcilation.
#[allow(clippy::needless_pass_by_value)]
fn on_error(_: Arc<Service>, error: &LBTrackerError, _context: Arc<CurrentContext>) -> Action {
    match error {
        LBTrackerError::SkipService => Action::await_change(),
        _ => Action::requeue(Duration::from_secs(30)),
    }
}
