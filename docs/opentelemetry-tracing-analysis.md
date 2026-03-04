# OpenTelemetry Tracing Integration Trade-off Analysis

**Issue:** #51
**Date:** 2026-03-04
**Status:** Analysis

## Executive Summary

This document analyzes the trade-offs of adding OpenTelemetry distributed tracing support to RobotLB. The project already uses OpenTelemetry for metrics and the `tracing` crate for structured logging, making the integration relatively straightforward.

## Current State

### Existing Observability Stack

| Component | Implementation | Purpose |
|-----------|---------------|---------|
| Metrics | OpenTelemetry SDK 0.31 + Custom Prometheus Exporter | Operational metrics (reconcile ops, API calls, leader status) |
| Logging | `tracing` + `tracing-subscriber` | Structured logging with log levels |
| Tracing | `#[tracing::instrument]` (logs only) | Function-level instrumentation (logged, not exported) |

### Current Dependencies (Cargo.toml)

```toml
opentelemetry = "0.31"
opentelemetry_sdk = { version = "0.31", features = ["rt-tokio", "metrics"] }
tracing = "0.1.40"
tracing-subscriber = "0.3.18"
```

## Proposed Integration

### Option A: Full OTLP Export (Recommended)

Add distributed tracing with OTLP (OpenTelemetry Protocol) export to compatible backends (Jaeger, Tempo, SigNoz, etc.).

**New Dependencies:**
```toml
opentelemetry-otlp = { version = "0.31", features = ["trace", "grpc"] }
opentelemetry-semantic-conventions = "0.31"
tracing-opentelemetry = "0.30"  # Bridge tracing crate to OTel
```

**Architecture:**
```
Application Code
       ↓
   tracing crate
       ↓
tracing-opentelemetry (bridge layer)
       ↓
opentelemetry-otlp (exporter)
       ↓
OTLP Collector / Backend (Jaeger, Tempo, etc.)
```

### Option B: In-Process Jaeger Export

Direct export to Jaeger without OTLP collector.

**New Dependencies:**
```toml
opentelemetry-jaeger = "0.31"  # Note: deprecated in favor of OTLP
tracing-opentelemetry = "0.30"
```

### Option C: Opt-in with Multiple Exporters

Support multiple backends via configuration (OTLP, stdout for debugging).

## Trade-off Analysis

### Benefits

| Benefit | Impact | Description |
|---------|--------|-------------|
| **End-to-end visibility** | High | Trace requests from Kubernetes API through reconciliation to Hetzner API calls |
| **Performance debugging** | High | Identify slow reconciliations, API bottlenecks |
| **Error correlation** | High | Link errors across service boundaries |
| **Unified observability** | Medium | Same stack for metrics, logs, traces |
| **Existing foundation** | High | Already using `tracing` crate; minimal code changes |
| **Kubernetes ecosystem** | High | Standard in cloud-native deployments |
| **Vendor neutral** | Medium | OTLP works with Jaeger, Tempo, SigNoz, Datadog, etc. |

### Costs

| Cost | Impact | Description |
|------|--------|-------------|
| **Binary size increase** | Low | ~500KB-1MB additional |
| **Runtime overhead** | Low | ~1-3% CPU when enabled, negligible when disabled |
| **Dependency complexity** | Medium | 3-4 new crates |
| **Configuration burden** | Medium | New environment variables/options |
| **Operational requirements** | Medium | Need OTLP collector/backend |
| **Learning curve** | Low | Team familiar with tracing concepts |

### Specific Considerations for RobotLB

#### Where Tracing Adds Value

1. **Reconciliation Flow** (`src/controller/mod.rs`)
   - Trace full reconciliation lifecycle
   - Identify which services take longest to reconcile
   - Correlate Hetzner API calls with Kubernetes events

2. **Hetzner API Calls** (`src/lb/api.rs`)
   - Track API latency per operation
   - Identify rate limiting or slow responses
   - Attribute errors to specific operations

3. **Leader Election** (`src/main.rs`)
   - Trace election cycles
   - Debug failover scenarios

4. **Node Discovery** (`src/controller/nodes.rs`)
   - Track time spent discovering target nodes
   - Identify slow pod lookups

#### Example Instrumentation Points

```rust
// Already exists in controller/mod.rs
#[tracing::instrument(skip(svc, context), fields(service = svc.name_any()))]
pub async fn reconcile_service(...) -> RobotLBResult<Action>

// Would benefit from spans in lb/api.rs
pub async fn create_load_balancer(...) -> RobotLBResult<LoadBalancer> {
    // Span already created by tracing::info! but not exported
}

// Network operations in lb/api.rs
pub async fn attach_to_network(...)
```

### Configuration Options

Recommended environment variables:

```bash
# Enable/disable tracing (default: disabled)
ROBOTLB_TRACING_ENABLED=false

# OTLP endpoint (e.g., http://tempo:4317 for gRPC, http://tempo:4318 for HTTP)
ROBOTLB_OTLP_ENDPOINT=http://localhost:4317

# Sampling ratio (1.0 = all traces, 0.1 = 10%)
ROBOTLB_TRACING_SAMPLE_RATIO=1.0

# Service name for traces
ROBOTLB_SERVICE_NAME=robotlb
```

### Implementation Effort

| Task | Effort | Description |
|------|--------|-------------|
| Add dependencies | 5 min | Update Cargo.toml |
| Create tracing module | 1-2 hours | Initialize OTLP exporter |
| Add configuration | 30 min | CLI args/env vars |
| Instrument key functions | 2-3 hours | Add spans where missing |
| Update Helm chart | 30 min | Add config options |
| Documentation | 1 hour | Update README/tutorial |
| **Total** | **5-7 hours** | |

## Recommendations

### Recommended Approach: Option A (Full OTLP Export)

**Rationale:**
1. Already using OpenTelemetry for metrics
2. OTLP is the standard protocol with wide backend support
3. `tracing-opentelemetry` bridge requires minimal code changes
4. Can be disabled by default with zero overhead

### Implementation Phases

#### Phase 1: Core Integration (MVP)
- Add OTLP exporter with opt-in configuration
- Bridge existing `tracing` instrumentation
- Export traces when `ROBOTLB_TRACING_ENABLED=true`

#### Phase 2: Enhanced Instrumentation
- Add spans for Hetzner API calls
- Add span attributes for load balancer operations
- Include Kubernetes resource metadata in spans

#### Phase 3: Advanced Features
- Add span events for state changes
- Correlate traces with metrics
- Add baggage for cross-service correlation (if needed)

### Security Considerations

| Concern | Mitigation |
|---------|------------|
| Sensitive data in traces | Sanitize HCLOUD token from span attributes |
| Network exposure | Use internal OTLP endpoints only |
| Storage costs | Use sampling for high-traffic clusters |

### Backward Compatibility

- Tracing disabled by default (zero impact)
- No breaking changes to existing functionality
- Opt-in via environment variable

## Comparison: With vs Without Tracing

| Scenario | Without Tracing | With Tracing |
|----------|----------------|--------------|
| Debug slow reconciliation | Check logs, grep timestamps | Visual timeline with duration breakdown |
| API rate limit issues | Aggregate metrics only | Per-request latency distribution |
| Multi-service debugging | Correlate logs manually | Distributed trace across services |
| Performance regression | Compare metric averages | Identify specific slow operations |

## Decision Matrix

| Factor | Weight | No Action | Add Tracing |
|--------|--------|-----------|-------------|
| Debugging capability | 3 | 1 | 3 |
| Operational overhead | 2 | 3 | 2 |
| Implementation effort | 2 | 3 | 2 |
| Ecosystem alignment | 2 | 1 | 3 |
| **Weighted Score** | | **14** | **21** |

## Conclusion

**Recommendation: Proceed with OpenTelemetry tracing integration.**

The benefits significantly outweigh the costs:
- Low implementation effort (~5-7 hours)
- Leverages existing `tracing` infrastructure
- Opt-in by default means zero overhead for users who don't need it
- Aligns with Kubernetes/cloud-native best practices
- Provides critical debugging capabilities for production issues

The project is well-positioned for this integration since it already uses the `tracing` crate extensively. Adding distributed tracing is primarily a configuration and exporter setup task rather than a major code refactoring effort.

## Next Steps

1. [ ] Approve integration approach
2. [ ] Create feature branch
3. [ ] Implement Phase 1 (Core Integration)
4. [ ] Add Helm chart values for tracing configuration
5. [ ] Update documentation
6. [ ] Create example Grafana Tempo/Jaeger deployment

## References

- [OpenTelemetry Rust Documentation](https://docs.rs/opentelemetry/)
- [tracing-opentelemetry Integration](https://docs.rs/tracing-opentelemetry/)
- [OTLP Specification](https://opentelemetry.io/docs/specs/otlp/)
- [Grafana Tempo](https://grafana.com/oss/tempo/)
- [Jaeger](https://www.jaegertracing.io/)
