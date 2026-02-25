use k8s_openapi::{api::core::v1::Service, serde_json::json};
use kube::{
    api::{Patch, PatchParams},
    Api, Client, ResourceExt,
};

use crate::{
    consts,
    error::{RobotLBError, RobotLBResult},
};

const MAX_FINALIZER_PATCH_RETRIES: usize = 3;

#[derive(Clone, Copy)]
enum FinalizerOperation {
    Add,
    Remove,
}

fn add_robotlb_finalizer(existing: &[String]) -> Vec<String> {
    if existing.iter().any(|item| item == consts::FINALIZER_NAME) {
        return existing.to_vec();
    }

    let mut desired = existing.to_vec();
    desired.push(consts::FINALIZER_NAME.to_string());
    desired
}

fn remove_robotlb_finalizer(existing: &[String]) -> Vec<String> {
    existing
        .iter()
        .filter(|item| item.as_str() != consts::FINALIZER_NAME)
        .cloned()
        .collect()
}

fn desired_finalizers(existing: &[String], operation: FinalizerOperation) -> Vec<String> {
    match operation {
        FinalizerOperation::Add => add_robotlb_finalizer(existing),
        FinalizerOperation::Remove => remove_robotlb_finalizer(existing),
    }
}

async fn apply_patch(
    api: &Api<Service>,
    service_name: &str,
    finalizers: &[String],
    resource_version: Option<&str>,
) -> RobotLBResult<()> {
    let patch = match resource_version {
        Some(resource_version) => json!({
            "metadata": {
                "resourceVersion": resource_version,
                "finalizers": finalizers,
            }
        }),
        None => json!({
            "metadata": {
                "finalizers": finalizers,
            }
        }),
    };

    api.patch(
        service_name,
        &PatchParams::default(),
        &Patch::Merge(&patch),
    )
    .await?;
    Ok(())
}

async fn reconcile_finalizers(
    client: Client,
    svc: &Service,
    operation: FinalizerOperation,
) -> RobotLBResult<()> {
    let namespace = svc.namespace().ok_or(RobotLBError::SkipService)?;
    let service_name = svc.name_any();
    let api = Api::<Service>::namespaced(client, namespace.as_str());

    for _ in 0..MAX_FINALIZER_PATCH_RETRIES {
        let latest = api.get(service_name.as_str()).await?;
        let desired = desired_finalizers(latest.finalizers(), operation);
        if desired == latest.finalizers() {
            return Ok(());
        }

        let resource_version = latest.metadata.resource_version.as_deref();
        match apply_patch(&api, service_name.as_str(), &desired, resource_version).await {
            Ok(()) => return Ok(()),
            Err(RobotLBError::KubeError(kube::Error::Api(error))) if error.code == 409 => {
                continue;
            }
            Err(err) => return Err(err),
        }
    }

    let latest = api.get(service_name.as_str()).await?;
    let desired = desired_finalizers(latest.finalizers(), operation);
    if desired == latest.finalizers() {
        return Ok(());
    }

    apply_patch(&api, service_name.as_str(), &desired, None).await
}

/// Add finalizer to the service.
/// This will prevent the service from being deleted.
pub async fn add(client: Client, svc: &Service) -> RobotLBResult<()> {
    reconcile_finalizers(client, svc, FinalizerOperation::Add).await
}

/// Check if service has the finalizer.
#[must_use]
pub fn check(service: &Service) -> bool {
    service
        .metadata
        .finalizers
        .as_ref()
        .is_some_and(|finalizers| finalizers.contains(&consts::FINALIZER_NAME.to_string()))
}

/// Remove finalizer from the service.
/// This will allow the service to be deleted.
///
/// if service does not have the finalizer, this function will do nothing.
pub async fn remove(client: Client, svc: &Service) -> RobotLBResult<()> {
    reconcile_finalizers(client, svc, FinalizerOperation::Remove).await
}

#[cfg(test)]
mod tests {
    use super::{add_robotlb_finalizer, remove_robotlb_finalizer};

    #[test]
    fn add_finalizer_is_idempotent_and_keeps_existing_entries() {
        let existing = vec!["kubernetes.io/foo".to_string()];
        let after_add = add_robotlb_finalizer(&existing);
        assert_eq!(after_add, vec!["kubernetes.io/foo", "robotlb/finalizer"]);

        let after_add_again = add_robotlb_finalizer(&after_add);
        assert_eq!(after_add_again, after_add);
    }

    #[test]
    fn remove_finalizer_is_idempotent_and_keeps_non_robotlb_entries() {
        let existing = vec![
            "robotlb/finalizer".to_string(),
            "kubernetes.io/foo".to_string(),
            "robotlb/finalizer".to_string(),
        ];

        let after_remove = remove_robotlb_finalizer(&existing);
        assert_eq!(after_remove, vec!["kubernetes.io/foo"]);

        let after_remove_again = remove_robotlb_finalizer(&after_remove);
        assert_eq!(after_remove_again, after_remove);
    }
}
