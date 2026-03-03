//! Prometheus metrics for the robotlb operator.
//!
//! This module provides metrics for monitoring the operator's performance
//! and the state of managed load balancers.

use std::{sync::Arc, time::Instant};

use prometheus::{
    CounterVec, GaugeVec, Opts, Registry, TextEncoder, register_counter_vec_with_registry,
    register_gauge_vec_with_registry,
};

use crate::consts;

const METRICS_PREFIX: &str = "robotlb";

pub struct Metrics {
    registry: Registry,
    reconcile_total: CounterVec,
    reconcile_failures: CounterVec,
    reconcile_duration: GaugeVec,
    services_managed: GaugeVec,
    hcloud_api_requests_total: CounterVec,
    hcloud_api_errors_total: CounterVec,
    leader_status: GaugeVec,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    #[must_use]
    pub fn new() -> Self {
        let registry = Registry::new();

        let reconcile_total = register_counter_vec_with_registry!(
            Opts::new(
                format!("{METRICS_PREFIX}_reconcile_operations_total"),
                "Total number of reconcile operations"
            ),
            &["controller"],
            registry
        )
        .expect("create reconcile_total counter");

        let reconcile_failures = register_counter_vec_with_registry!(
            Opts::new(
                format!("{METRICS_PREFIX}_reconcile_failures_total"),
                "Total number of failed reconcile operations"
            ),
            &["controller"],
            registry
        )
        .expect("create reconcile_failures counter");

        let reconcile_duration = register_gauge_vec_with_registry!(
            Opts::new(
                format!("{METRICS_PREFIX}_reconcile_duration_seconds"),
                "Duration of the last reconcile operation"
            ),
            &["controller"],
            registry
        )
        .expect("create reconcile_duration gauge");

        let services_managed = register_gauge_vec_with_registry!(
            Opts::new(
                format!("{METRICS_PREFIX}_services_managed"),
                "Number of services currently managed by the operator"
            ),
            &["controller"],
            registry
        )
        .expect("create services_managed gauge");

        let hcloud_api_requests_total = register_counter_vec_with_registry!(
            Opts::new(
                format!("{METRICS_PREFIX}_hcloud_api_requests_total"),
                "Total number of Hetzner Cloud API requests"
            ),
            &["controller"],
            registry
        )
        .expect("create hcloud_api_requests_total counter");

        let hcloud_api_errors_total = register_counter_vec_with_registry!(
            Opts::new(
                format!("{METRICS_PREFIX}_hcloud_api_errors_total"),
                "Total number of Hetzner Cloud API errors"
            ),
            &["controller"],
            registry
        )
        .expect("create hcloud_api_errors_total counter");

        let leader_status = register_gauge_vec_with_registry!(
            Opts::new(
                format!("{METRICS_PREFIX}_leader_status"),
                "Whether this instance is the leader (1=leader, 0=not leader)"
            ),
            &["controller"],
            registry
        )
        .expect("create leader_status gauge");

        Self {
            registry,
            reconcile_total,
            reconcile_failures,
            reconcile_duration,
            services_managed,
            hcloud_api_requests_total,
            hcloud_api_errors_total,
            leader_status,
        }
    }

    const fn controller_label() -> [&'static str; 1] {
        [consts::ROBOTLB_LB_CLASS]
    }

    pub fn inc_reconcile_total(&self) {
        self.reconcile_total
            .with_label_values(&Self::controller_label())
            .inc();
    }

    pub fn inc_reconcile_failures(&self) {
        self.reconcile_failures
            .with_label_values(&Self::controller_label())
            .inc();
    }

    pub fn observe_reconcile_duration(&self, duration: std::time::Duration) {
        let secs = duration.as_secs_f64();
        self.reconcile_duration
            .with_label_values(&Self::controller_label())
            .set(secs);
    }

    pub fn set_services_managed(&self, count: u64) {
        self.services_managed
            .with_label_values(&Self::controller_label())
            .set(f64::from(u32::try_from(count).unwrap_or(u32::MAX)));
    }

    pub fn inc_hcloud_api_requests(&self) {
        self.hcloud_api_requests_total
            .with_label_values(&Self::controller_label())
            .inc();
    }

    pub fn inc_hcloud_api_errors(&self) {
        self.hcloud_api_errors_total
            .with_label_values(&Self::controller_label())
            .inc();
    }

    pub fn set_leader_status(&self, is_leader: bool) {
        self.leader_status
            .with_label_values(&Self::controller_label())
            .set(f64::from(is_leader));
    }

    #[must_use]
    pub fn export(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder
            .encode_to_string(&metric_families)
            .unwrap_or_default()
    }
}

pub struct ReconcileTimer {
    metrics: Arc<Metrics>,
    start: Instant,
    failed: bool,
}

impl ReconcileTimer {
    #[must_use]
    pub fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            metrics,
            start: Instant::now(),
            failed: false,
        }
    }

    pub const fn set_failed(&mut self) {
        self.failed = true;
    }
}

impl Drop for ReconcileTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.metrics.observe_reconcile_duration(duration);
        self.metrics.inc_reconcile_total();
        if self.failed {
            self.metrics.inc_reconcile_failures();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_export() {
        let metrics = Metrics::new();
        metrics.inc_reconcile_total();
        metrics.inc_reconcile_total();
        metrics.inc_reconcile_failures();
        metrics.set_services_managed(5);
        metrics.set_leader_status(true);

        let output = metrics.export();
        assert!(output.contains("robotlb_reconcile_operations_total"));
        assert!(output.contains("robotlb_reconcile_failures_total"));
        assert!(output.contains("robotlb_services_managed"));
        assert!(output.contains("robotlb_leader_status"));
    }
}
