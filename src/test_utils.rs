//! Shared test utilities for the robotlb operator.
//!
//! This module provides common test helpers to reduce duplication
//! across test modules.

#[cfg(test)]
pub mod fixtures {
    use k8s_openapi::{
        api::core::v1::{Service, ServicePort, ServiceSpec},
        apimachinery::pkg::apis::meta::v1::ObjectMeta,
    };

    #[must_use]
    pub fn service_with_spec(spec: ServiceSpec) -> Service {
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

    pub fn service_spec(
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
}
