use thiserror::Error;

pub type LBTrackerResult<T> = Result<T, LBTrackerError>;

#[derive(Debug, Error)]
pub enum LBTrackerError {
    #[error("Cannot parse node filter: {0}")]
    InvalidNodeFilter(String),
    #[error("Unsupported service type")]
    UnsupportedServiceType,
    #[error("Service was skipped")]
    SkipService,
    #[error("Cannot parse integer value: {0}")]
    PaseIntError(#[from] std::num::ParseIntError),
    #[error("Cannot parse boolean value: {0}")]
    PaseBoolError(#[from] std::str::ParseBoolError),
    #[error("HCloud error: {0}")]
    HCloudError(String),
    #[error("Kube error: {0}")]
    KubeError(#[from] kube::Error),
    #[error("Unknown LoadBalancing alorithm")]
    UnknownLBAlgorithm,
    #[error("Cannot get target nodes, because the service has no selector")]
    ServiceWithoutSelector,

    // HCloud API errors
    #[error("Cannot attach load balancer to a network. Reason: {0}")]
    HCloudLBAttachToNetworkError(
        #[from]
        hcloud::apis::Error<hcloud::apis::load_balancers_api::AttachLoadBalancerToNetworkError>,
    ),
    #[error("Cannot detach load balancer from network. Reason: {0}")]
    HcloudLBDetachFromNetworkError(
        #[from]
        hcloud::apis::Error<hcloud::apis::load_balancers_api::DetachLoadBalancerFromNetworkError>,
    ),
    #[error("Cannot add load balancer target. Reason: {0}")]
    HcloudLBAddTargetError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::AddTargetError>,
    ),
    #[error("Cannot remove load balancer target. Reason: {0}")]
    HcloudLBRemoveTargetError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::RemoveTargetError>,
    ),
    #[error("Cannot add service to load balancer. Reason: {0}")]
    HcloudLBAddServiceError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::AddServiceError>,
    ),
    #[error("Cannot remove service from load balancer. Reason: {0}")]
    HcloudLBRemoveServiceError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::DeleteServiceError>,
    ),
    #[error("Cannot create load balancer. Reason: {0}")]
    HcloudLBCreateError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::CreateLoadBalancerError>,
    ),
    #[error("Cannot delete load balancer. Reason: {0}")]
    HcloudLBDeleteError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::DeleteLoadBalancerError>,
    ),
    #[error("Cannot get load balancer. Reason: {0}")]
    HcloudLBGetError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::GetLoadBalancerError>,
    ),
    #[error("Cannot update service. Reason: {0}")]
    HcloudLBUpdateServiceError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::UpdateServiceError>,
    ),
    #[error("Cannot change type of load balancer. Reason: {0}")]
    HcloudLBChangeType(
        #[from]
        hcloud::apis::Error<hcloud::apis::load_balancers_api::ChangeTypeOfLoadBalancerError>,
    ),
    #[error("Cannot change algorithm of load balancer. Reason: {0}")]
    HcloudLBChangeAlgorithm(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::ChangeAlgorithmError>,
    ),
    #[error("Cannot list networks. Reason: {0}")]
    HcloudListNetworksError(
        #[from] hcloud::apis::Error<hcloud::apis::networks_api::ListNetworksError>,
    ),
    #[error("Cannot list load balancers. Reason: {0}")]
    HcloudListLoadBalancersError(
        #[from] hcloud::apis::Error<hcloud::apis::load_balancers_api::ListLoadBalancersError>,
    ),
}
