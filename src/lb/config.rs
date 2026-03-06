//! Configuration parsing for load balancers.
//!
//! This module handles parsing Kubernetes service annotations into
//! load balancer configuration.

use std::str::FromStr;

use k8s_openapi::api::core::v1::Service;
use kube::ResourceExt;

use crate::{
    config::OperatorConfig,
    consts,
    error::{RobotLBError, RobotLBResult},
};

use super::types::{LBAlgorithm, ParsedLoadBalancerConfig};

/// Parse load balancer configuration from a Kubernetes service.
///
/// This function extracts configuration from service annotations,
/// falling back to operator defaults when annotations are not present.
///
/// # Errors
///
/// Returns an error if annotation values cannot be parsed or if
/// an invalid algorithm is specified.
pub fn parse_load_balancer_config(
    svc: &Service,
    config: &OperatorConfig,
) -> RobotLBResult<ParsedLoadBalancerConfig> {
    let retries =
        parse_annotation(svc, consts::LB_RETRIES_ANNOTATION)?.unwrap_or(config.default_lb_retries);

    let timeout =
        parse_annotation(svc, consts::LB_TIMEOUT_ANNOTATION)?.unwrap_or(config.default_lb_timeout);

    let check_interval = parse_annotation(svc, consts::LB_CHECK_INTERVAL_ANNOTATION)?
        .unwrap_or(config.default_lb_interval);

    let proxy_mode = parse_annotation(svc, consts::LB_PROXY_MODE_ANNOTATION)?
        .unwrap_or(config.default_lb_proxy_mode_enabled);

    let location = svc
        .annotations()
        .get(consts::LB_LOCATION_ANNOTATION)
        .cloned()
        .unwrap_or_else(|| config.default_lb_location.clone());

    let balancer_type = svc
        .annotations()
        .get(consts::LB_BALANCER_TYPE_ANNOTATION)
        .cloned()
        .unwrap_or_else(|| config.default_balancer_type.clone());

    let algorithm = parse_algorithm(svc, config)?;

    let network_name = svc
        .annotations()
        .get(consts::LB_NETWORK_ANNOTATION)
        .or(config.default_network.as_ref())
        .cloned();

    let name = svc
        .annotations()
        .get(consts::LB_NAME_ANNOTATION)
        .cloned()
        .unwrap_or_else(|| svc.name_any());

    let private_ip = svc
        .annotations()
        .get(consts::LB_PRIVATE_IP_ANNOTATION)
        .cloned();

    Ok(ParsedLoadBalancerConfig {
        name,
        private_ip,
        balancer_type,
        check_interval,
        timeout,
        retries,
        location,
        proxy_mode,
        network_name,
        algorithm: algorithm.into(),
    })
}

/// Parse a numeric or boolean annotation from a service.
fn parse_annotation<T>(svc: &Service, key: &str) -> RobotLBResult<Option<T>>
where
    T: FromStr,
    RobotLBError: From<T::Err>,
{
    svc.annotations()
        .get(key)
        .map(String::as_str)
        .map(T::from_str)
        .transpose()
        .map_err(Into::into)
}

/// Parse the algorithm annotation or fall back to default.
fn parse_algorithm(svc: &Service, config: &OperatorConfig) -> RobotLBResult<LBAlgorithm> {
    svc.annotations()
        .get(consts::LB_ALGORITHM_ANNOTATION)
        .map(String::as_str)
        .or(Some(&config.default_lb_algorithm))
        .map(LBAlgorithm::from_str)
        .transpose()?
        .ok_or(RobotLBError::UnknownLBAlgorithm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OperatorConfig;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;
    use tracing::level_filters::LevelFilter;

    fn base_config() -> OperatorConfig {
        OperatorConfig {
            hcloud_token: "token".to_string(),
            default_network: Some("default-net".to_string()),
            dynamic_node_selector: true,
            default_lb_retries: 3,
            default_lb_timeout: 10,
            default_lb_interval: 15,
            default_lb_location: "hel1".to_string(),
            default_balancer_type: "lb11".to_string(),
            default_lb_algorithm: "least-connections".to_string(),
            default_lb_proxy_mode_enabled: false,
            ipv6_ingress: false,
            leader_election_namespace: None,
            leader_election_lease_name: "robotlb-leader-election".to_string(),
            leader_election_lease_ttl_secs: 15,
            leader_election_renew_interval_secs: 5,
            log_level: LevelFilter::INFO,
            tracing_enabled: false,
            otlp_endpoint: "http://localhost:4317".to_string(),
            tracing_sample_ratio: 1.0,
            service_name: "robotlb".to_string(),
        }
    }

    fn service_with_annotations(
        annotations: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Service {
        let annotation_map = annotations
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>();

        Service {
            metadata: ObjectMeta {
                name: Some("svc-name".to_string()),
                annotations: Some(annotation_map),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn uses_defaults_when_annotations_are_missing() {
        let config = base_config();
        let svc = service_with_annotations([]);

        let parsed = parse_load_balancer_config(&svc, &config).expect("parse should succeed");

        assert_eq!(parsed.name, "svc-name");
        assert_eq!(parsed.retries, 3);
        assert_eq!(parsed.timeout, 10);
        assert_eq!(parsed.check_interval, 15);
        assert_eq!(parsed.location, "hel1");
        assert_eq!(parsed.balancer_type, "lb11");
        assert_eq!(parsed.network_name.as_deref(), Some("default-net"));
        assert!(!parsed.proxy_mode);
    }

    #[test]
    fn parses_annotations_into_load_balancer_config() {
        let mut config = base_config();
        config.default_network = None;
        let svc = service_with_annotations([
            (consts::LB_NAME_ANNOTATION, "custom-lb"),
            (consts::LB_RETRIES_ANNOTATION, "5"),
            (consts::LB_TIMEOUT_ANNOTATION, "8"),
            (consts::LB_CHECK_INTERVAL_ANNOTATION, "20"),
            (consts::LB_PROXY_MODE_ANNOTATION, "true"),
            (consts::LB_LOCATION_ANNOTATION, "nbg1"),
            (consts::LB_BALANCER_TYPE_ANNOTATION, "lb31"),
            (consts::LB_ALGORITHM_ANNOTATION, "round-robin"),
            (consts::LB_NETWORK_ANNOTATION, "private-net"),
            (consts::LB_PRIVATE_IP_ANNOTATION, "10.10.0.5"),
        ]);

        let parsed = parse_load_balancer_config(&svc, &config).expect("parse should succeed");

        assert_eq!(parsed.name, "custom-lb");
        assert_eq!(parsed.retries, 5);
        assert_eq!(parsed.timeout, 8);
        assert_eq!(parsed.check_interval, 20);
        assert_eq!(parsed.location, "nbg1");
        assert_eq!(parsed.balancer_type, "lb31");
        assert_eq!(parsed.network_name.as_deref(), Some("private-net"));
        assert_eq!(parsed.private_ip.as_deref(), Some("10.10.0.5"));
        assert!(parsed.proxy_mode);
        assert_eq!(
            parsed.algorithm.r#type,
            hcloud::models::load_balancer_algorithm::Type::RoundRobin
        );
    }

    #[test]
    fn returns_error_for_invalid_algorithm_annotation() {
        let config = base_config();
        let svc = service_with_annotations([(consts::LB_ALGORITHM_ANNOTATION, "weighted")]);

        let result = parse_load_balancer_config(&svc, &config);
        assert!(matches!(result, Err(RobotLBError::UnknownLBAlgorithm)));
    }
}
