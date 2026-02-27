//! Kubernetes controller for managing load balancer services.
//!
//! This module contains the reconciliation logic for Kubernetes services
//! with the `robotlb` load balancer class.

mod nodes;
mod status;

use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use futures::StreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::{
    Resource, ResourceExt,
    runtime::{Controller, controller::Action, reflector::ObjectRef, watcher},
};

use crate::{
    CurrentContext, consts,
    error::{RobotLBError, RobotLBResult},
    finalizers,
    lb::LoadBalancer,
};

pub(crate) use nodes::{
    apply_desired_state, derive_services, derive_targets, discover_target_nodes, node_ip_type,
};
pub(crate) use status::{build_ingress, patch_ingress_status};

const SUCCESS_REQUEUE_SECS: u64 = 180;

/// Check if a service is supported by robotlb.
///
/// Returns an error if the service is not a `LoadBalancer` type or
/// if the load balancer class is not `robotlb`.
///
/// # Errors
///
/// Returns `SkipService` if the service is not supported.
pub fn ensure_service_is_supported(svc: &Service) -> RobotLBResult<()> {
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

/// Reconcile the service.
///
/// This function is called by the controller for each service.
/// It will create or update the load balancer based on the service.
/// If the service is being deleted, it will clean up the resources.
///
/// # Errors
///
/// Returns an error if the service is not supported, or if the Kubernetes/Hetzner API call fails.
#[tracing::instrument(skip(svc, context), fields(service = svc.name_any()))]
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

/// Reconcile the `LoadBalancer` type of service.
///
/// This function will find the nodes based on the node selector
/// and create or update the load balancer.
///
/// # Errors
///
/// Returns an error if node discovery fails, or if the Kubernetes/Hetzner API call fails.
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

    let ingress = build_ingress(&hcloud_lb, context.config.ipv6_ingress, lb.proxy_mode);
    patch_ingress_status(&svc, &context, ingress).await?;

    Ok(Action::requeue(Duration::from_secs(SUCCESS_REQUEUE_SECS)))
}

/// Handle the error during reconciliation.
#[allow(clippy::needless_pass_by_value)]
pub fn on_error(_: Arc<Service>, error: &RobotLBError, _context: Arc<CurrentContext>) -> Action {
    action_for_error(error)
}

const fn action_for_error(error: &RobotLBError) -> Action {
    match error {
        RobotLBError::SkipService => Action::await_change(),
        _ => Action::requeue(Duration::from_secs(30)),
    }
}

/// Map an `EndpointSlice` to the parent Service.
#[must_use]
pub fn map_endpoint_slice_to_service(
    endpoint_slice: &k8s_openapi::api::discovery::v1::EndpointSlice,
) -> Option<ObjectRef<Service>> {
    let namespace = endpoint_slice.namespace()?;
    let service_name = endpoint_slice
        .metadata
        .labels
        .as_ref()?
        .get("kubernetes.io/service-name")?
        .clone();

    Some(ObjectRef::new(&service_name).within(&namespace))
}

/// Create and run the controller.
pub async fn run(
    kube_client: kube::Client,
    context: Arc<CurrentContext>,
    shutdown: tokio_util::sync::CancellationToken,
) {
    let controller_shutdown = shutdown.clone();
    Controller::new(
        kube::Api::<Service>::all(kube_client),
        watcher::Config::default(),
    )
    .watches(
        kube::Api::<k8s_openapi::api::discovery::v1::EndpointSlice>::all(context.client.clone()),
        watcher::Config::default(),
        |endpoint_slice| map_endpoint_slice_to_service(&endpoint_slice),
    )
    .run(reconcile_service, on_error, context)
    .take_until(controller_shutdown.cancelled())
    .for_each(|reconcilation_result| async move {
        match reconcilation_result {
            Ok((service, _action)) => {
                tracing::info!("Reconcilation of a service {} was successful", service.name);
            }
            Err(err) => match err {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::{
        api::core::v1::{ServicePort, ServiceSpec},
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
