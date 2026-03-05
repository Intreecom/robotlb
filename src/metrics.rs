use std::{sync::Arc, time::Instant};

use opentelemetry::{
    metrics::{Counter, Gauge, Histogram, Meter},
    KeyValue,
};
use tokio::time::Instant as TokioInstant;
use tracing::debug;

use crate::consts;

pub struct Metrics {
    controller: String,
    reconcile_operations: Counter<u64>,
    reconcile_failures: Counter<u64>,
    reconcile_duration: Histogram<f64>,
    services_managed: Gauge<i64>,
    hcloud_api_requests: Counter<u64>,
    hcloud_api_errors: Counter<u64>,
    leader_status: Gauge<i64>,
}

impl Metrics {
    pub fn new(meter: &Meter) -> Self {
        debug!("Initializing robotlb metrics");

        let reconcile_operations = meter
            .u64_counter("reconcile_operations")
            .with_description("Total number of reconcile operations")
            .build();

        let reconcile_failures = meter
            .u64_counter("reconcile_failures")
            .with_description("Total number of failed reconcile operations")
            .build();

        let reconcile_duration = meter
            .f64_histogram("reconcile_duration_seconds")
            .with_description("Duration of reconcile operations")
            .with_boundaries(vec![0.001, 0.01, 0.1, 1.0, 10.0])
            .build();

        let services_managed = meter
            .i64_gauge("services_managed")
            .with_description("Number of services currently managed by the operator")
            .build();

        let hcloud_api_requests = meter
            .u64_counter("hcloud_api_requests")
            .with_description("Total number of Hetzner Cloud API requests")
            .build();

        let hcloud_api_errors = meter
            .u64_counter("hcloud_api_errors")
            .with_description("Total number of Hetzner Cloud API errors")
            .build();

        let leader_status = meter
            .i64_gauge("leader_status")
            .with_description("Whether this instance is the leader (1=leader, 0=not leader)")
            .build();

        debug!("Robotlb metrics initialized");

        Self {
            controller: consts::ROBOTLB_LB_CLASS.to_string(),
            reconcile_operations,
            reconcile_failures,
            reconcile_duration,
            services_managed,
            hcloud_api_requests,
            hcloud_api_errors,
            leader_status,
        }
    }

    fn controller_label(&self) -> [KeyValue; 1] {
        [KeyValue::new("controller", self.controller.clone())]
    }

    pub fn inc_reconcile_operations(&self) {
        self.reconcile_operations.add(1, &self.controller_label());
    }

    pub fn inc_reconcile_failures(&self) {
        self.reconcile_failures.add(1, &self.controller_label());
    }

    pub fn record_reconcile_duration(&self, duration: f64) {
        self.reconcile_duration
            .record(duration, &self.controller_label());
    }

    pub fn set_services_managed(&self, count: i64) {
        self.services_managed
            .record(count, &self.controller_label());
    }

    pub fn inc_hcloud_api_requests(&self) {
        self.hcloud_api_requests.add(1, &self.controller_label());
    }

    pub fn inc_hcloud_api_errors(&self) {
        self.hcloud_api_errors.add(1, &self.controller_label());
    }

    pub fn set_leader_status(&self, is_leader: bool) {
        self.leader_status
            .record(i64::from(is_leader), &self.controller_label());
    }

    #[must_use]
    pub fn count_and_measure(&self) -> ReconcileMeasurer {
        self.inc_reconcile_operations();
        ReconcileMeasurer {
            start: TokioInstant::now(),
            controller: self.controller.clone(),
            metric: self.reconcile_duration.clone(),
        }
    }
}

pub struct ReconcileMeasurer {
    start: TokioInstant,
    controller: String,
    metric: Histogram<f64>,
}

impl Drop for ReconcileMeasurer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.metric.record(
            duration,
            &[KeyValue::new("controller", self.controller.clone())],
        );
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
        let duration = self.start.elapsed().as_secs_f64();
        self.metrics.record_reconcile_duration(duration);
        self.metrics.inc_reconcile_operations();
        if self.failed {
            self.metrics.inc_reconcile_failures();
        }
    }
}
