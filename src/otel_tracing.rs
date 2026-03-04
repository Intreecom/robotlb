//! OpenTelemetry tracing initialization.
//!
//! This module provides distributed tracing support via OpenTelemetry Protocol (OTLP).
//! Tracing is opt-in and disabled by default for zero overhead.

use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{self, Sampler, SdkTracerProvider},
    Resource,
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use crate::config::OperatorConfig;

pub struct TracingGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl TracingGuard {
    pub const fn empty() -> Self {
        Self {
            tracer_provider: None,
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(provider) = self.tracer_provider.take() {
            let _ = provider.shutdown();
        }
    }
}

pub fn init_tracing(config: &OperatorConfig) -> Result<TracingGuard, trace::TraceError> {
    let fmt_layer =
        tracing_subscriber::fmt::layer().with_filter(LevelFilter::from(config.log_level));

    if !config.tracing_enabled {
        tracing_subscriber::registry().with(fmt_layer).init();
        return Ok(TracingGuard::empty());
    }

    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .build();

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(format!("{}/v1/traces", config.otlp_endpoint))
        .build()
        .map_err(|e| trace::TraceError::from(e.to_string()))?;

    let sampler = if (config.tracing_sample_ratio - 1.0).abs() < f64::EPSILON {
        Sampler::AlwaysOn
    } else {
        Sampler::TraceIdRatioBased(config.tracing_sample_ratio)
    };

    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_sampler(sampler)
        .with_batch_exporter(exporter)
        .build();

    let tracer = tracer_provider.tracer("robotlb");

    global::set_tracer_provider(tracer_provider.clone());

    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(LevelFilter::from(config.log_level));

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(telemetry_layer)
        .init();

    Ok(TracingGuard {
        tracer_provider: Some(tracer_provider),
    })
}
