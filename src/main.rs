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
use error::{RobotLBError, RobotLBResult};
use futures::StreamExt;
use hcloud::apis::configuration::Configuration as HCloudConfig;
use k8s_openapi::{
    api::core::v1::{Node, Pod, Service},
    api::discovery::v1::EndpointSlice,
    serde_json::{json, Value},
};
use kube::{
    api::{ListParams, PatchParams},
    runtime::{controller::Action, reflector::ObjectRef, watcher, Controller},
    Resource, ResourceExt,
};
use kube_leader_election::{LeaseLock, LeaseLockParams, LeaseLockResult};
use label_filter::LabelFilter;
use lb::LoadBalancer;
use std::{
    collections::HashSet,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const SUCCESS_REQUEUE_SECS: u64 = 180;

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
async fn main() -> RobotLBResult<()> {
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
    let is_leader = Arc::new(AtomicBool::new(false));
    spawn_leader_election_task(kube_client.clone(), &operator_config, is_leader.clone());

    let context = Arc::new(CurrentContext::new(
        kube_client.clone(),
        operator_config.clone(),
        hcloud_conf,
        is_leader,
    ));
    tracing::info!("Starting the controller");
    Controller::new(
        kube::Api::<Service>::all(kube_client),
        watcher::Config::default(),
    )
    .watches(
        kube::Api::<EndpointSlice>::all(context.client.clone()),
        watcher::Config::default(),
        |endpoint_slice| map_endpoint_slice_to_service(&endpoint_slice),
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
                    RobotLBError::SkipService,
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
    pub is_leader: Arc<AtomicBool>,
}
impl CurrentContext {
    #[must_use]
    pub const fn new(
        client: kube::Client,
        config: OperatorConfig,
        hcloud_config: HCloudConfig,
        is_leader: Arc<AtomicBool>,
    ) -> Self {
        Self {
            client,
            config,
            hcloud_config,
            is_leader,
        }
    }
}

fn ensure_service_is_supported(svc: &Service) -> RobotLBResult<()> {
    let svc_type = svc
        .spec
        .as_ref()
        .and_then(|spec| spec.type_.as_ref())
        .map_or("ClusterIP", String::as_str);
    if svc_type != "LoadBalancer" {
        tracing::debug!(
            service_type = svc_type,
            "Service type is not LoadBalancer. Skipping..."
        );
        return Err(RobotLBError::SkipService);
    }

    let lb_class = svc
        .spec
        .as_ref()
        .and_then(|spec| spec.load_balancer_class.as_ref())
        .map_or(consts::ROBOTLB_LB_CLASS, String::as_str);
    if lb_class != consts::ROBOTLB_LB_CLASS {
        tracing::debug!(
            load_balancer_class = lb_class,
            "Load balancer class is not robotlb. Skipping..."
        );
        return Err(RobotLBError::SkipService);
    }

    Ok(())
}

const fn node_ip_type(lb: &LoadBalancer) -> &'static str {
    if lb.network_name.is_none() {
        "ExternalIP"
    } else {
        "InternalIP"
    }
}

fn derive_targets(nodes: Vec<Node>, desired_ip_type: &str) -> Vec<String> {
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

fn derive_services(svc: &Service) -> Vec<(i32, i32)> {
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

async fn discover_target_nodes(
    svc: &Arc<Service>,
    context: &Arc<CurrentContext>,
) -> RobotLBResult<Vec<Node>> {
    if context.config.dynamic_node_selector {
        get_nodes_dynamically(svc, context).await
    } else {
        get_nodes_by_selector(svc, context).await
    }
}

fn apply_desired_state(
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

fn build_ingress(hcloud_lb: &hcloud::models::LoadBalancer, enable_ipv6: bool) -> Vec<Value> {
    let mut ingress = vec![];

    let dns_ipv4 = hcloud_lb.public_net.ipv4.dns_ptr.clone().flatten();
    let ipv4 = hcloud_lb.public_net.ipv4.ip.clone().flatten();
    let dns_ipv6 = hcloud_lb.public_net.ipv6.dns_ptr.clone().flatten();
    let ipv6 = hcloud_lb.public_net.ipv6.ip.clone().flatten();

    if let Some(ipv4) = &ipv4 {
        ingress.push(json!({
            "ip": ipv4,
            "dns": dns_ipv4,
            "ip_mode": "VIP"
        }));
    }

    if enable_ipv6 {
        if let Some(ipv6) = &ipv6 {
            ingress.push(json!({
                "ip": ipv6,
                "dns": dns_ipv6,
                "ip_mode": "VIP"
            }));
        }
    }

    ingress
}

async fn patch_ingress_status(
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
            &kube::api::Patch::Merge(json!({
                "status" :{
                    "loadBalancer": {
                        "ingress": ingress
                    }
                }
            })),
        )
        .await?;

    Ok(())
}

/// Reconcile the service.
/// This function is called by the controller for each service.
/// It will create or update the load balancer based on the service.
/// If the service is being deleted, it will clean up the resources.
#[tracing::instrument(skip(svc,context), fields(service=svc.name_any()))]
pub async fn reconcile_service(
    svc: Arc<Service>,
    context: Arc<CurrentContext>,
) -> RobotLBResult<Action> {
    if !context.is_leader.load(Ordering::Relaxed) {
        return Err(RobotLBError::SkipService);
    }

    ensure_service_is_supported(&svc)?;

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

/// Get nodes based on the node selector.
/// This method will find the nodes based on the node selector
/// from the service annotations.
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

/// Reconcile the `LoadBalancer` type of service.
/// This function will find the nodes based on the node selector
/// and create or update the load balancer.
pub async fn reconcile_load_balancer(
    mut lb: LoadBalancer,
    svc: Arc<Service>,
    context: Arc<CurrentContext>,
) -> RobotLBResult<Action> {
    let desired_ip_type = node_ip_type(&lb);

    let nodes = discover_target_nodes(&svc, &context).await?;
    let desired_targets = derive_targets(nodes, desired_ip_type);
    let desired_services = derive_services(&svc);

    apply_desired_state(&mut lb, desired_targets, desired_services);

    let hcloud_lb = lb.reconcile().await?;

    let ingress = build_ingress(&hcloud_lb, context.config.ipv6_ingress);
    patch_ingress_status(&svc, &context, ingress).await?;

    Ok(Action::requeue(Duration::from_secs(SUCCESS_REQUEUE_SECS)))
}

/// Handle the error during reconcilation.
#[allow(clippy::needless_pass_by_value)]
fn on_error(_: Arc<Service>, error: &RobotLBError, _context: Arc<CurrentContext>) -> Action {
    action_for_error(error)
}

const fn action_for_error(error: &RobotLBError) -> Action {
    match error {
        RobotLBError::SkipService => Action::await_change(),
        _ => Action::requeue(Duration::from_secs(30)),
    }
}

fn map_endpoint_slice_to_service(endpoint_slice: &EndpointSlice) -> Option<ObjectRef<Service>> {
    let namespace = endpoint_slice.namespace()?;
    let service_name = endpoint_slice
        .metadata
        .labels
        .as_ref()?
        .get("kubernetes.io/service-name")?
        .clone();

    Some(ObjectRef::new(&service_name).within(&namespace))
}

fn spawn_leader_election_task(
    client: kube::Client,
    config: &OperatorConfig,
    is_leader: Arc<AtomicBool>,
) {
    let lease_namespace = config
        .leader_election_namespace
        .clone()
        .unwrap_or_else(detect_runtime_namespace);
    let holder_id =
        std::env::var("HOSTNAME").unwrap_or_else(|_| format!("robotlb-{}", std::process::id()));
    let lease = LeaseLock::new(
        client,
        &lease_namespace,
        LeaseLockParams {
            holder_id: holder_id.clone(),
            lease_name: config.leader_election_lease_name.clone(),
            lease_ttl: Duration::from_secs(config.leader_election_lease_ttl_secs),
        },
    );
    let renew_interval = Duration::from_secs(config.leader_election_renew_interval_secs);

    tracing::info!(
        lease_name = %config.leader_election_lease_name,
        lease_namespace = %lease_namespace,
        holder_id = %holder_id,
        "Starting leader election"
    );

    tokio::spawn(async move {
        loop {
            match lease.try_acquire_or_renew().await {
                Ok(LeaseLockResult::Acquired(_)) => {
                    if !is_leader.load(Ordering::Relaxed) {
                        tracing::info!("Leadership acquired");
                    }
                    is_leader.store(true, Ordering::Relaxed);
                }
                Ok(LeaseLockResult::NotAcquired(_)) => {
                    if is_leader.load(Ordering::Relaxed) {
                        tracing::warn!("Leadership lost");
                    }
                    is_leader.store(false, Ordering::Relaxed);
                }
                Err(err) => {
                    if is_leader.load(Ordering::Relaxed) {
                        tracing::warn!(error = %err, "Leader election failed, stepping down");
                    } else {
                        tracing::warn!(error = %err, "Leader election attempt failed");
                    }
                    is_leader.store(false, Ordering::Relaxed);
                }
            }
            tokio::time::sleep(renew_interval).await;
        }
    });
}

fn detect_runtime_namespace() -> String {
    std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace")
        .ok()
        .map(|namespace| namespace.trim().to_string())
        .filter(|namespace| !namespace.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

#[cfg(test)]
mod tests {
    use super::{action_for_error, derive_services, derive_targets, ensure_service_is_supported};
    use crate::error::RobotLBError;
    use k8s_openapi::{
        api::core::v1::{Node, NodeAddress, NodeStatus, Service, ServicePort, ServiceSpec},
        apimachinery::pkg::apis::meta::v1::ObjectMeta,
    };
    use kube::runtime::controller::Action;
    use std::time::Duration;

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
    fn service_filter_rejects_non_load_balancer_type() {
        let svc = service_with_spec(service_spec("ClusterIP", None, vec![]));
        let result = ensure_service_is_supported(&svc);
        assert!(matches!(result, Err(RobotLBError::SkipService)));
    }

    #[test]
    fn service_filter_rejects_foreign_load_balancer_class() {
        let svc = service_with_spec(service_spec(
            "LoadBalancer",
            Some("other-controller"),
            vec![],
        ));
        let result = ensure_service_is_supported(&svc);
        assert!(matches!(result, Err(RobotLBError::SkipService)));
    }

    #[test]
    fn service_filter_accepts_default_robotlb_class() {
        let svc = service_with_spec(service_spec("LoadBalancer", None, vec![]));
        assert!(ensure_service_is_supported(&svc).is_ok());
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

    #[test]
    fn on_error_awaits_change_for_skipped_service() {
        let action = action_for_error(&RobotLBError::SkipService);
        assert_eq!(action, Action::await_change());
    }

    #[test]
    fn on_error_requeues_for_non_skip_errors() {
        let action = action_for_error(&RobotLBError::ServiceWithoutSelector);
        assert_eq!(action, Action::requeue(Duration::from_secs(30)));
    }
}
