use clap::Parser;
use tracing::level_filters::LevelFilter;

#[derive(Debug, Clone, Parser)]
pub struct OperatorConfig {
    /// `HCloud` API token.
    #[arg(short = 't', long, env = "LB_TRACKER_HCLOUD_TOKEN")]
    pub hcloud_token: String,

    /// Default load balancer healthcheck retries cound.
    #[arg(long, env = "LB_TRACKER_DEFAULT_LB_RETRIES", default_value = "3")]
    pub default_lb_retries: i32,

    /// Default load balancer healthcheck timeout.
    #[arg(long, env = "LB_TRACKER_DEFAULT_LB_TIMEOUT", default_value = "10")]
    pub default_lb_timeout: i32,

    /// Default load balancer healhcheck interval.
    #[arg(long, env = "LB_TRACKER_DEFAULT_LB_INTERVAL", default_value = "15")]
    pub default_lb_interval: i32,

    /// Default loadction of a load balancer.
    /// https://docs.hetzner.com/cloud/general/locations/
    #[arg(long, env = "LB_TRACKER_DEFAULT_LB_LOCATION", default_value = "hel1")]
    pub default_lb_location: String,

    /// Type of a load balancer. It differs in price, number of connections,
    /// target servers, etc. The default value is the smallest balancer.
    /// https://docs.hetzner.com/cloud/load-balancers/overview#pricing
    #[arg(long, env = "LB_TRACKER_DEFAULT_LB_TYPE", default_value = "lb11")]
    pub default_balancer_type: String,

    /// Default load balancer algorithm.
    /// Possible values:
    /// * `least-connections`
    /// * `round-robin`
    /// https://docs.hetzner.com/cloud/load-balancers/overview#load-balancers
    #[arg(
        long,
        env = "LB_TRACKER_DEFAULT_LB_ALGORITHM",
        default_value = "least-connections"
    )]
    pub default_lb_algorithm: String,

    /// Default load balancer proxy mode. If enabled, the load balancer will
    /// act as a proxy for the target servers. The default value is `false`.
    /// https://docs.hetzner.com/cloud/load-balancers/faq/#what-does-proxy-protocol-mean-and-should-i-enable-it
    #[arg(
        long,
        env = "LB_TRACKER_DEFAULT_LB_PROXY_MODE_ENABLED",
        default_value = "false"
    )]
    pub default_lb_proxy_mode_enabled: bool,

    /// Whether to enable IPv6 ingress for the load balancer. The default value
    /// is `false`. If enabled, the load balancer's IPv6 will be attached to a
    /// service as external IP along with IPv4.
    #[arg(long, env = "LB_TRACKER_IPV6_INGRESS", default_value = "false")]
    pub ipv6_ingress: bool,

    // Log level for the operator.
    #[arg(long, env = "LB_TRACKER_LOG_LEVEL", default_value = "INFO")]
    pub log_level: LevelFilter,
}
