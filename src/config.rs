//! Configuration types for the robotlb operator.
//!
//! This module defines the command-line arguments and environment variables
//! used to configure the operator at runtime.

use clap::Parser;
use tracing::level_filters::LevelFilter;

#[derive(Debug, Clone, Parser)]
#[allow(clippy::struct_excessive_bools)]
pub struct OperatorConfig {
    /// `HCloud` API token.
    #[arg(short = 't', long, env = "ROBOTLB_HCLOUD_TOKEN")]
    pub hcloud_token: String,

    /// Default network to use for load balancers.
    /// If not set, then only network from the service annotation will be used.
    #[arg(long, env = "ROBOTLB_DEFAULT_NETWORK", default_value = None)]
    pub default_network: Option<String>,

    /// If enabled, the operator will try to find target nodes based on where the target pods are actually deployed.
    /// If disabled, the operator will try to find target nodes based on the node selector.
    #[arg(long, env = "ROBOTLB_DYNAMIC_NODE_SELECTOR", default_value = "true")]
    pub dynamic_node_selector: bool,

    /// Default load balancer healthcheck retries cound.
    #[arg(long, env = "ROBOTLB_DEFAULT_LB_RETRIES", default_value = "3")]
    pub default_lb_retries: i32,

    /// Default load balancer healthcheck timeout.
    #[arg(long, env = "ROBOTLB_DEFAULT_LB_TIMEOUT", default_value = "10")]
    pub default_lb_timeout: i32,

    /// Default load balancer healhcheck interval.
    #[arg(long, env = "ROBOTLB_DEFAULT_LB_INTERVAL", default_value = "15")]
    pub default_lb_interval: i32,

    /// Default location of a load balancer.
    /// <https://docs.hetzner.com/cloud/general/locations/>
    #[arg(long, env = "ROBOTLB_DEFAULT_LB_LOCATION", default_value = "hel1")]
    pub default_lb_location: String,

    /// Type of a load balancer. It differs in price, number of connections,
    /// target servers, etc. The default value is the smallest balancer.
    /// <https://docs.hetzner.com/cloud/load-balancers/overview#pricing>
    #[arg(long, env = "ROBOTLB_DEFAULT_LB_TYPE", default_value = "lb11")]
    pub default_balancer_type: String,

    /// Default load balancer algorithm.
    /// Possible values:
    /// * `least-connections`
    /// * `round-robin`
    ///
    /// <https://docs.hetzner.com/cloud/load-balancers/overview#load-balancers>
    #[arg(
        long,
        env = "ROBOTLB_DEFAULT_LB_ALGORITHM",
        default_value = "least-connections"
    )]
    pub default_lb_algorithm: String,

    /// Default load balancer proxy mode. If enabled, the load balancer will
    /// act as a proxy for the target servers. The default value is `false`.
    /// <https://docs.hetzner.com/cloud/load-balancers/faq/#what-does-proxy-protocol-mean-and-should-i-enable-it>
    #[arg(
        long,
        env = "ROBOTLB_DEFAULT_LB_PROXY_MODE_ENABLED",
        default_value = "false"
    )]
    pub default_lb_proxy_mode_enabled: bool,

    /// Whether to enable IPv6 ingress for the load balancer.
    /// If enabled, the load balancer's IPv6 will be attached to the service as an external IP along with IPv4.
    #[arg(long, env = "ROBOTLB_IPV6_INGRESS", default_value = "false")]
    pub ipv6_ingress: bool,

    /// Optional namespace for the leader election lease.
    /// If not set, robotlb will auto-detect its runtime namespace.
    #[arg(long, env = "ROBOTLB_LEADER_ELECTION_NAMESPACE", default_value = None)]
    pub leader_election_namespace: Option<String>,

    /// Name of the Kubernetes Lease object used for leader election.
    #[arg(
        long,
        env = "ROBOTLB_LEADER_ELECTION_LEASE_NAME",
        default_value = "robotlb-leader-election"
    )]
    pub leader_election_lease_name: String,

    /// Lease TTL in seconds.
    #[arg(
        long,
        env = "ROBOTLB_LEADER_ELECTION_LEASE_TTL_SECS",
        default_value = "15"
    )]
    pub leader_election_lease_ttl_secs: u64,

    /// How often to attempt lease acquire/renew in seconds.
    #[arg(
        long,
        env = "ROBOTLB_LEADER_ELECTION_RENEW_INTERVAL_SECS",
        default_value = "5"
    )]
    pub leader_election_renew_interval_secs: u64,

    // Log level of the operator.
    #[arg(long, env = "ROBOTLB_LOG_LEVEL", default_value = "INFO")]
    pub log_level: LevelFilter,

    // Enable distributed tracing via OpenTelemetry.
    #[arg(long, env = "ROBOTLB_TRACING_ENABLED", default_value = "false")]
    pub tracing_enabled: bool,

    // OTLP endpoint for trace export (e.g., http://tempo:4317).
    #[arg(
        long,
        env = "ROBOTLB_OTLP_ENDPOINT",
        default_value = "http://localhost:4317"
    )]
    pub otlp_endpoint: String,

    // Sampling ratio for traces (1.0 = all, 0.1 = 10%).
    #[arg(long, env = "ROBOTLB_TRACING_SAMPLE_RATIO", default_value = "1.0")]
    pub tracing_sample_ratio: f64,

    // Service name for traces.
    #[arg(long, env = "ROBOTLB_SERVICE_NAME", default_value = "robotlb")]
    pub service_name: String,
}

#[cfg(test)]
mod tests {
    use super::OperatorConfig;
    use clap::Parser;
    use tracing::level_filters::LevelFilter;

    #[test]
    fn parses_defaults_from_cli() {
        let config = OperatorConfig::try_parse_from(["robotlb", "--hcloud-token", "token"])
            .expect("config should parse");

        assert_eq!(config.hcloud_token, "token");
        assert_eq!(config.default_network, None);
        assert!(config.dynamic_node_selector);
        assert_eq!(config.default_lb_retries, 3);
        assert_eq!(config.default_lb_timeout, 10);
        assert_eq!(config.default_lb_interval, 15);
        assert_eq!(config.default_lb_location, "hel1");
        assert_eq!(config.default_balancer_type, "lb11");
        assert_eq!(config.default_lb_algorithm, "least-connections");
        assert!(!config.default_lb_proxy_mode_enabled);
        assert!(!config.ipv6_ingress);
        assert_eq!(config.leader_election_namespace, None);
        assert_eq!(config.leader_election_lease_name, "robotlb-leader-election");
        assert_eq!(config.leader_election_lease_ttl_secs, 15);
        assert_eq!(config.leader_election_renew_interval_secs, 5);
        assert_eq!(config.log_level, LevelFilter::INFO);
        assert!(!config.tracing_enabled);
        assert_eq!(config.otlp_endpoint, "http://localhost:4317");
        assert!((config.tracing_sample_ratio - 1.0).abs() < f64::EPSILON);
        assert_eq!(config.service_name, "robotlb");
    }

    #[test]
    fn parses_explicit_overrides_from_cli() {
        let config = OperatorConfig::try_parse_from([
            "robotlb",
            "--hcloud-token",
            "token",
            "--default-network",
            "prod-net",
            "--default-lb-retries",
            "5",
            "--default-lb-timeout",
            "7",
            "--default-lb-interval",
            "9",
            "--default-lb-location",
            "nbg1",
            "--default-balancer-type",
            "lb21",
            "--default-lb-algorithm",
            "round-robin",
            "--default-lb-proxy-mode-enabled",
            "--ipv6-ingress",
            "--leader-election-namespace",
            "robotlb",
            "--leader-election-lease-name",
            "robotlb-ha-lock",
            "--leader-election-lease-ttl-secs",
            "30",
            "--leader-election-renew-interval-secs",
            "10",
            "--log-level",
            "DEBUG",
            "--tracing-enabled",
            "--otlp-endpoint",
            "http://tempo:4317",
            "--tracing-sample-ratio",
            "0.5",
            "--service-name",
            "robotlb-prod",
        ])
        .expect("config should parse");

        assert_eq!(config.default_network.as_deref(), Some("prod-net"));
        assert!(config.dynamic_node_selector);
        assert_eq!(config.default_lb_retries, 5);
        assert_eq!(config.default_lb_timeout, 7);
        assert_eq!(config.default_lb_interval, 9);
        assert_eq!(config.default_lb_location, "nbg1");
        assert_eq!(config.default_balancer_type, "lb21");
        assert_eq!(config.default_lb_algorithm, "round-robin");
        assert!(config.default_lb_proxy_mode_enabled);
        assert!(config.ipv6_ingress);
        assert_eq!(config.leader_election_namespace.as_deref(), Some("robotlb"));
        assert_eq!(config.leader_election_lease_name, "robotlb-ha-lock");
        assert_eq!(config.leader_election_lease_ttl_secs, 30);
        assert_eq!(config.leader_election_renew_interval_secs, 10);
        assert_eq!(config.log_level, LevelFilter::DEBUG);
        assert!(config.tracing_enabled);
        assert_eq!(config.otlp_endpoint, "http://tempo:4317");
        assert!((config.tracing_sample_ratio - 0.5).abs() < f64::EPSILON);
        assert_eq!(config.service_name, "robotlb-prod");
    }
}
