pub const LB_NAME_LABEL_NAME: &str = "robotlb/balancer";
pub const LB_NODE_SELECTOR: &str = "robotlb/node-selector";
pub const LB_NODE_IP_LABEL_NAME: &str = "robotlb/node-ip";

// LB config
pub const LB_CHECK_INTERVAL_ANN_NAME: &str = "robotlb/lb-check-interval";
pub const LB_TIMEOUT_ANN_NAME: &str = "robotlb/lb-timeout";
pub const LB_RETRIES_ANN_NAME: &str = "robotlb/lb-retries";
pub const LB_PROXY_MODE_LABEL_NAME: &str = "robotlb/lb-proxy-mode";
pub const LB_NETWORK_LABEL_NAME: &str = "robotlb/lb-network";

pub const LB_LOCATION_LABEL_NAME: &str = "robotlb/lb-location";
pub const LB_ALGORITHM_LABEL_NAME: &str = "robotlb/lb-algorithm";
pub const LB_BALANCER_TYPE_LABEL_NAME: &str = "robotlb/balancer-type";

pub const DEFAULT_LB_RETRIES: i32 = 3;
pub const DEFAULT_LB_TIMEOUT: i32 = 10;
pub const DEFAULT_LB_INTERVAL: i32 = 15;

pub const DEFAULT_LB_LOCATION: &str = "hel1";
pub const DEFAULT_LB_ALGORITHM: &str = "least-connections";
pub const DEFAULT_LB_BALANCER_TYPE: &str = "lb11";

pub const FINALIZER_NAME: &str = "robotlb/finalizer";
