use std::{
    fmt::Write as FmtWrite,
    sync::{Arc, Mutex},
    time::Duration,
};

use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    error::OTelSdkError,
    metrics::{
        Temporality,
        data::{AggregatedMetrics, Gauge, Histogram, MetricData, ResourceMetrics, Sum},
        exporter::PushMetricExporter,
    },
};
use tracing::debug;

#[derive(Clone)]
pub struct PrometheusExporter {
    data: Arc<Mutex<Option<String>>>,
}

impl PrometheusExporter {
    pub fn new() -> Self {
        debug!("Creating new Prometheus exporter");
        Self {
            data: Arc::new(Mutex::new(None)),
        }
    }

    /// # Panics
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn get_metrics(&self) -> Option<String> {
        self.data.lock().unwrap().clone()
    }

    fn format_metrics(metrics: &ResourceMetrics) -> String {
        let mut output = String::new();

        for scope_metrics in metrics.scope_metrics() {
            let scope_name = scope_metrics.scope().name();
            for metric in scope_metrics.metrics() {
                let metric_name = format!("{}_{}", scope_name, metric.name());
                let description = metric.description();

                writeln!(output, "# HELP {metric_name} {description}").ok();

                match metric.data() {
                    AggregatedMetrics::F64(data) => {
                        Self::format_metric_data(&metric_name, data, &mut output, |v| v);
                    }
                    AggregatedMetrics::U64(data) => {
                        #[allow(clippy::cast_precision_loss)]
                        Self::format_metric_data(&metric_name, data, &mut output, |v| v as f64);
                    }
                    AggregatedMetrics::I64(data) => {
                        #[allow(clippy::cast_precision_loss)]
                        Self::format_metric_data(&metric_name, data, &mut output, |v| v as f64);
                    }
                }
            }
        }

        writeln!(output, "# EOF").ok();
        output
    }

    fn format_metric_data<T>(
        metric_name: &str,
        data: &MetricData<T>,
        output: &mut String,
        to_f64: impl Fn(T) -> f64,
    ) where
        T: Copy,
    {
        match data {
            MetricData::Sum(sum) => {
                writeln!(output, "# TYPE {metric_name} counter").ok();
                Self::format_sum(metric_name, sum, output, to_f64);
            }
            MetricData::Gauge(gauge) => {
                writeln!(output, "# TYPE {metric_name} gauge").ok();
                Self::format_gauge(metric_name, gauge, output, to_f64);
            }
            MetricData::Histogram(histogram) => {
                writeln!(output, "# TYPE {metric_name} histogram").ok();
                Self::format_histogram(metric_name, histogram, output, to_f64);
            }
            MetricData::ExponentialHistogram(_) => {}
        }
    }

    fn format_sum<T>(
        metric_name: &str,
        sum: &Sum<T>,
        output: &mut String,
        to_f64: impl Fn(T) -> f64,
    ) where
        T: Copy,
    {
        for data_point in sum.data_points() {
            let labels = format_attributes(data_point.attributes());
            let value = to_f64(data_point.value());
            writeln!(output, "{metric_name}_total{labels} {value}").ok();
        }
    }

    fn format_gauge<T>(
        metric_name: &str,
        gauge: &Gauge<T>,
        output: &mut String,
        to_f64: impl Fn(T) -> f64,
    ) where
        T: Copy,
    {
        for data_point in gauge.data_points() {
            let labels = format_attributes(data_point.attributes());
            let value = to_f64(data_point.value());
            writeln!(output, "{metric_name}{labels} {value}").ok();
        }
    }

    fn format_histogram<T>(
        metric_name: &str,
        histogram: &Histogram<T>,
        output: &mut String,
        to_f64: impl Fn(T) -> f64,
    ) where
        T: Copy,
    {
        for data_point in histogram.data_points() {
            let labels_base = format_attributes(data_point.attributes());

            let bounds_vec: Vec<_> = data_point.bounds().collect();
            let mut cumulative = 0u64;
            for (i, count) in data_point.bucket_counts().enumerate() {
                cumulative += count;
                let bucket_label = if i < bounds_vec.len() {
                    format!("le=\"{}\"", bounds_vec[i])
                } else {
                    "le=\"+Inf\"".to_string()
                };

                let labels = if labels_base.is_empty() {
                    format!("{{{bucket_label}}}")
                } else {
                    format!(
                        "{},{}}}",
                        &labels_base[..labels_base.len() - 1],
                        bucket_label
                    )
                };

                writeln!(output, "{metric_name}_bucket{labels} {cumulative}").ok();
            }

            writeln!(
                output,
                "{metric_name}_sum{labels_base} {}",
                to_f64(data_point.sum())
            )
            .ok();
            writeln!(
                output,
                "{metric_name}_count{labels_base} {}",
                data_point.count()
            )
            .ok();
        }
    }
}

impl Default for PrometheusExporter {
    fn default() -> Self {
        Self::new()
    }
}

fn format_attributes<'a>(attrs: impl Iterator<Item = &'a KeyValue>) -> String {
    let collected: Vec<_> = attrs.collect();
    if collected.is_empty() {
        return String::new();
    }

    let mut labels = String::from("{");
    for (i, kv) in collected.iter().enumerate() {
        if i > 0 {
            labels.push(',');
        }
        write!(labels, "{}=\"{}\"", kv.key.as_str(), kv.value).ok();
    }
    labels.push('}');
    labels
}

impl PushMetricExporter for PrometheusExporter {
    async fn export(&self, metrics: &ResourceMetrics) -> Result<(), OTelSdkError> {
        let formatted = Self::format_metrics(metrics);
        *self.data.lock().unwrap() = Some(formatted);
        Ok(())
    }

    fn force_flush(&self) -> Result<(), OTelSdkError> {
        Ok(())
    }

    fn shutdown(&self) -> Result<(), OTelSdkError> {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> Result<(), OTelSdkError> {
        Ok(())
    }

    fn temporality(&self) -> Temporality {
        Temporality::Cumulative
    }
}

static PROMETHEUS_EXPORTER: Mutex<Option<PrometheusExporter>> = Mutex::new(None);

/// # Panics
/// Panics if the global mutex is poisoned.
pub fn set_global_exporter(exporter: PrometheusExporter) {
    debug!("Setting global Prometheus exporter");
    *PROMETHEUS_EXPORTER.lock().unwrap() = Some(exporter);
}

/// # Errors
/// Returns an error string if the Prometheus exporter is not initialized or if no metrics are available yet.
///
/// # Panics
/// Panics if the global mutex is poisoned.
pub fn format_prometheus_metrics() -> Result<String, String> {
    let exporter_guard = PROMETHEUS_EXPORTER.lock().unwrap();

    exporter_guard.as_ref().map_or_else(
        || Err("Prometheus exporter not initialized".to_string()),
        |exporter| {
            exporter
                .get_metrics()
                .ok_or_else(|| "No metrics available yet".to_string())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::metrics::{Meter, MeterProvider};

    fn create_test_provider_and_exporter() -> (
        opentelemetry_sdk::metrics::SdkMeterProvider,
        PrometheusExporter,
        Meter,
    ) {
        let exporter = PrometheusExporter::new();
        let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter.clone())
            .with_interval(std::time::Duration::from_millis(50))
            .build();
        let provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
            .with_reader(reader)
            .build();
        let meter = provider.meter("test");
        (provider, exporter, meter)
    }

    async fn wait_for_export() {
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn test_counter_export() {
        let (_provider, exporter, meter) = create_test_provider_and_exporter();

        let counter = meter
            .u64_counter("test_counter")
            .with_description("A test counter")
            .build();

        counter.add(5, &[KeyValue::new("label1", "value1")]);
        counter.add(3, &[KeyValue::new("label1", "value2")]);

        wait_for_export().await;

        let metrics = exporter.get_metrics();
        assert!(metrics.is_some(), "Metrics should be available");

        let metrics_text = metrics.unwrap();
        assert!(metrics_text.contains("# HELP test_test_counter"));
        assert!(metrics_text.contains("# TYPE test_test_counter counter"));
    }

    #[tokio::test]
    async fn test_gauge_export() {
        let (_provider, exporter, meter) = create_test_provider_and_exporter();

        let gauge = meter
            .i64_gauge("test_gauge")
            .with_description("A test gauge")
            .build();

        gauge.record(42, &[KeyValue::new("status", "active")]);

        wait_for_export().await;

        let metrics = exporter.get_metrics();
        assert!(metrics.is_some(), "Metrics should be available");

        let metrics_text = metrics.unwrap();
        assert!(metrics_text.contains("# HELP test_test_gauge"));
        assert!(metrics_text.contains("# TYPE test_test_gauge gauge"));
    }

    #[test]
    fn test_format_attributes_empty() {
        let attrs: [KeyValue; 0] = [];
        let result = format_attributes(attrs.iter());
        assert_eq!(result, "");
    }

    #[test]
    fn test_format_attributes_single() {
        let attrs = [KeyValue::new("key", "value")];
        let result = format_attributes(attrs.iter());
        assert_eq!(result, r#"{key="value"}"#);
    }

    #[test]
    fn test_format_attributes_multiple() {
        let attrs = [
            KeyValue::new("key1", "value1"),
            KeyValue::new("key2", "value2"),
        ];
        let result = format_attributes(attrs.iter());
        assert_eq!(result, r#"{key1="value1",key2="value2"}"#);
    }
}
