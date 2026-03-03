//! Prometheus metrics for the robotlb operator.
//!
//! This module provides metrics for monitoring the operator's performance
//! and the state of managed load balancers.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use crate::consts;

const METRICS_PREFIX: &str = "robotlb";

pub struct Metrics {
    reconcile_total: AtomicU64,
    reconcile_failures: AtomicU64,
    reconcile_duration_secs: AtomicU64,
    reconcile_duration_nanos: AtomicU64,
    services_managed: AtomicU64,
    hcloud_api_requests_total: AtomicU64,
    hcloud_api_errors_total: AtomicU64,
    leader_status: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reconcile_total: AtomicU64::new(0),
            reconcile_failures: AtomicU64::new(0),
            reconcile_duration_secs: AtomicU64::new(0),
            reconcile_duration_nanos: AtomicU64::new(0),
            services_managed: AtomicU64::new(0),
            hcloud_api_requests_total: AtomicU64::new(0),
            hcloud_api_errors_total: AtomicU64::new(0),
            leader_status: AtomicU64::new(0),
        }
    }

    pub fn inc_reconcile_total(&self) {
        self.reconcile_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_reconcile_failures(&self) {
        self.reconcile_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn observe_reconcile_duration(&self, duration: std::time::Duration) {
        let secs = duration.as_secs();
        let nanos = u64::from(duration.subsec_nanos());
        self.reconcile_duration_secs.store(secs, Ordering::Relaxed);
        self.reconcile_duration_nanos
            .store(nanos, Ordering::Relaxed);
    }

    pub fn set_services_managed(&self, count: u64) {
        self.services_managed.store(count, Ordering::Relaxed);
    }

    pub fn inc_hcloud_api_requests(&self) {
        self.hcloud_api_requests_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_hcloud_api_errors(&self) {
        self.hcloud_api_errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_leader_status(&self, is_leader: bool) {
        self.leader_status
            .store(u64::from(is_leader), Ordering::Relaxed);
    }

    #[must_use]
    pub fn export(&self) -> String {
        let reconcile_total = self.reconcile_total.load(Ordering::Relaxed);
        let reconcile_failures = self.reconcile_failures.load(Ordering::Relaxed);
        let duration_secs = self.reconcile_duration_secs.load(Ordering::Relaxed);
        let duration_nanos = self.reconcile_duration_nanos.load(Ordering::Relaxed);
        let services_managed = self.services_managed.load(Ordering::Relaxed);
        let hcloud_api_requests = self.hcloud_api_requests_total.load(Ordering::Relaxed);
        let hcloud_api_errors = self.hcloud_api_errors_total.load(Ordering::Relaxed);
        let leader_status = self.leader_status.load(Ordering::Relaxed);

        let duration_total = f64::from(u32::try_from(duration_secs).unwrap_or(u32::MAX))
            + f64::from(u32::try_from(duration_nanos).unwrap_or(0)) / 1e9;

        format!(
            r#"# HELP {prefix}_reconcile_operations_total Total number of reconcile operations
# TYPE {prefix}_reconcile_operations_total counter
{prefix}_reconcile_operations_total{{controller="{controller}"}} {reconcile_total}

# HELP {prefix}_reconcile_failures_total Total number of failed reconcile operations
# TYPE {prefix}_reconcile_failures_total counter
{prefix}_reconcile_failures_total{{controller="{controller}"}} {reconcile_failures}

# HELP {prefix}_reconcile_duration_seconds Duration of the last reconcile operation
# TYPE {prefix}_reconcile_duration_seconds gauge
{prefix}_reconcile_duration_seconds{{controller="{controller}"}} {duration_total}

# HELP {prefix}_services_managed Number of services currently managed by the operator
# TYPE {prefix}_services_managed gauge
{prefix}_services_managed{{controller="{controller}"}} {services_managed}

# HELP {prefix}_hcloud_api_requests_total Total number of Hetzner Cloud API requests
# TYPE {prefix}_hcloud_api_requests_total counter
{prefix}_hcloud_api_requests_total{{controller="{controller}"}} {hcloud_api_requests}

# HELP {prefix}_hcloud_api_errors_total Total number of Hetzner Cloud API errors
# TYPE {prefix}_hcloud_api_errors_total counter
{prefix}_hcloud_api_errors_total{{controller="{controller}"}} {hcloud_api_errors}

# HELP {prefix}_leader_status Whether this instance is the leader (1=leader, 0=not leader)
# TYPE {prefix}_leader_status gauge
{prefix}_leader_status{{controller="{controller}"}} {leader_status}

"#,
            prefix = METRICS_PREFIX,
            controller = consts::ROBOTLB_LB_CLASS,
        )
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
        assert!(output.contains("robotlb_reconcile_operations_total{controller=\"robotlb\"} 2"));
        assert!(output.contains("robotlb_reconcile_failures_total{controller=\"robotlb\"} 1"));
        assert!(output.contains("robotlb_services_managed{controller=\"robotlb\"} 5"));
        assert!(output.contains("robotlb_leader_status{controller=\"robotlb\"} 1"));
    }
}
