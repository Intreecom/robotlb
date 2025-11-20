use gateway_api::apis::standard::gateways::Gateway;
use k8s_openapi::{api::core::v1::Service, serde_json::json};
use kube::{
    api::{Patch, PatchParams},
    Api, Client, ResourceExt,
};

use crate::{
    consts,
    error::{RobotLBError, RobotLBResult},
};

/// Add finalizer to the service.
/// This will prevent the service from being deleted.
pub async fn add(client: Client, svc: &Service) -> RobotLBResult<()> {
    let api = Api::<Service>::namespaced(
        client,
        svc.namespace().ok_or(RobotLBError::SkipService)?.as_str(),
    );
    let patch = json!({
        "metadata": {
            "finalizers": [consts::FINALIZER_NAME]
        }
    });
    api.patch(
        svc.name_any().as_str(),
        &PatchParams::default(),
        &Patch::Merge(patch),
    )
    .await?;
    Ok(())
}

/// Check if service has the finalizer.
#[must_use]
pub fn check(service: &Service) -> bool {
    service
        .metadata
        .finalizers
        .as_ref()
        .map_or(false, |finalizers| {
            finalizers.contains(&consts::FINALIZER_NAME.to_string())
        })
}

/// Remove finalizer from the service.
/// This will allow the service to be deleted.
///
/// if service does not have the finalizer, this function will do nothing.
pub async fn remove(client: Client, svc: &Service) -> RobotLBResult<()> {
    let api = Api::<Service>::namespaced(
        client,
        svc.namespace().ok_or(RobotLBError::SkipService)?.as_str(),
    );
    let finalizers = svc
        .finalizers()
        .iter()
        .filter(|item| item.as_str() != consts::FINALIZER_NAME)
        .collect::<Vec<_>>();
    let patch = json!({
        "metadata": {
            "finalizers": finalizers
        }
    });
    api.patch(
        svc.name_any().as_str(),
        &PatchParams::default(),
        &Patch::Merge(patch),
    )
    .await?;
    Ok(())
}

// Gateway-specific finalizer functions

/// Add finalizer to the Gateway.
/// This will prevent the Gateway from being deleted.
pub async fn add_gateway(client: Client, gateway: &Gateway) -> RobotLBResult<()> {
    let api = Api::<Gateway>::namespaced(
        client,
        gateway
            .namespace()
            .ok_or(RobotLBError::SkipGateway)?
            .as_str(),
    );
    let patch = json!({
        "metadata": {
            "finalizers": [consts::GATEWAY_FINALIZER_NAME]
        }
    });
    api.patch(
        gateway.name_any().as_str(),
        &PatchParams::default(),
        &Patch::Merge(patch),
    )
    .await?;
    Ok(())
}

/// Check if Gateway has the finalizer.
#[must_use]
pub fn check_gateway(gateway: &Gateway) -> bool {
    gateway
        .metadata
        .finalizers
        .as_ref()
        .map_or(false, |finalizers| {
            finalizers.contains(&consts::GATEWAY_FINALIZER_NAME.to_string())
        })
}

/// Remove finalizer from the Gateway.
/// This will allow the Gateway to be deleted.
///
/// if Gateway does not have the finalizer, this function will do nothing.
pub async fn remove_gateway(client: Client, gateway: &Gateway) -> RobotLBResult<()> {
    let api = Api::<Gateway>::namespaced(
        client,
        gateway
            .namespace()
            .ok_or(RobotLBError::SkipGateway)?
            .as_str(),
    );
    let finalizers = gateway
        .finalizers()
        .iter()
        .filter(|item| item.as_str() != consts::GATEWAY_FINALIZER_NAME)
        .collect::<Vec<_>>();
    let patch = json!({
        "metadata": {
            "finalizers": finalizers
        }
    });
    api.patch(
        gateway.name_any().as_str(),
        &PatchParams::default(),
        &Patch::Merge(patch),
    )
    .await?;
    Ok(())
}
