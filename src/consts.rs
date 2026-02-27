//! Constants used throughout the robotlb operator.
//!
//! This module defines annotation keys, label names, and default values
//! for load balancer configuration.

/// Annotation key for specifying the load balancer name.
pub const LB_NAME_LABEL_NAME: &str = "robotlb/balancer";
/// Annotation key for custom node selector.
pub const LB_NODE_SELECTOR: &str = "robotlb/node-selector";
/// Annotation key for specifying node IP.
pub const LB_NODE_IP_LABEL_NAME: &str = "robotlb/node-ip";

// LB config
/// Annotation key for health check interval (seconds).
pub const LB_CHECK_INTERVAL_ANN_NAME: &str = "robotlb/lb-check-interval";
/// Annotation key for health check timeout (seconds).
pub const LB_TIMEOUT_ANN_NAME: &str = "robotlb/lb-timeout";
/// Annotation key for health check retries.
pub const LB_RETRIES_ANN_NAME: &str = "robotlb/lb-retries";
/// Annotation key for enabling proxy mode.
pub const LB_PROXY_MODE_LABEL_NAME: &str = "robotlb/lb-proxy-mode";
/// Annotation key for network name.
pub const LB_NETWORK_LABEL_NAME: &str = "robotlb/lb-network";
/// Annotation key for private IP mode.
pub const LB_PRIVATE_IP_LABEL_NAME: &str = "robotlb/lb-private-ip";

/// Annotation key for load balancer location.
pub const LB_LOCATION_LABEL_NAME: &str = "robotlb/lb-location";
/// Annotation key for load balancing algorithm.
pub const LB_ALGORITHM_LABEL_NAME: &str = "robotlb/lb-algorithm";
/// Annotation key for load balancer type.
pub const LB_BALANCER_TYPE_LABEL_NAME: &str = "robotlb/balancer-type";

/// Default number of health check retries.
pub const DEFAULT_LB_RETRIES: i32 = 3;
/// Default health check timeout in seconds.
pub const DEFAULT_LB_TIMEOUT: i32 = 10;
/// Default health check interval in seconds.
pub const DEFAULT_LB_INTERVAL: i32 = 15;

/// Default load balancer location.
pub const DEFAULT_LB_LOCATION: &str = "hel1";
/// Default load balancing algorithm.
pub const DEFAULT_LB_ALGORITHM: &str = "least-connections";
/// Default load balancer type.
pub const DEFAULT_LB_BALANCER_TYPE: &str = "lb11";

/// Finalizer name used to prevent deletion before cleanup.
pub const FINALIZER_NAME: &str = "robotlb/finalizer";
/// Load balancer class name for Kubernetes services.
pub const ROBOTLB_LB_CLASS: &str = "robotlb";
