# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RobotLB is a Kubernetes operator written in Rust that integrates Hetzner Robot bare-metal clusters with Hetzner Cloud load balancers. It supports two APIs:
1. **LoadBalancer Services**: Traditional Kubernetes Service resources of type `LoadBalancer`
2. **Gateway API**: Modern Kubernetes Gateway API (v1) for more advanced routing capabilities

Both APIs provision and manage Hetzner Cloud load balancers automatically.

## Architecture

### Controller Pattern
The operator runs **two concurrent controllers** using the Kubernetes controller pattern via `kube-rs`:
1. **Service Controller**: Watches LoadBalancer Service resources
   - Main reconciliation in `src/main.rs:reconcile_service()`
   - Triggered on Service resource changes
2. **Gateway Controller**: Watches Gateway API resources
   - Main reconciliation in `src/main.rs:reconcile_gateway()`
   - Triggered on Gateway, HTTPRoute, and TCPRoute changes

Both controllers:
- Use finalizers to ensure clean resource deletion
- Requeue every 30 seconds for periodic reconciliation
- Run concurrently using `tokio::select!`

### Core Components

**main.rs**: Controller setup and service reconciliation
- `reconcile_service()`: Entry point for reconciliation, validates service type and load balancer class
- `reconcile_load_balancer()`: Main reconciliation logic that:
  - Determines target nodes (dynamically via pod locations or via node selector)
  - Collects node IPs (InternalIP for private networks, ExternalIP for public)
  - Extracts service ports and creates load balancer services
  - Updates Service status with load balancer ingress IPs
- `get_nodes_dynamically()`: Finds nodes where target pods are deployed using service selectors
- `get_nodes_by_selector()`: Finds nodes using label filters from annotations

**lb.rs**: Load balancer management
- `LoadBalancer` struct holds configuration parsed from service annotations and operator config
- `try_from_svc()`: Constructs LoadBalancer from Service annotations with fallback to operator defaults
- `reconcile()`: Orchestrates reconciliation of all LB aspects (algorithm, type, network, services, targets)
- `reconcile_services()`: Ensures LB services match desired configuration (add/update/delete)
- `reconcile_targets()`: Ensures LB targets match current node IPs (add/remove)
- `reconcile_network()`: Manages LB network attachment/detachment with optional private IP
- `reconcile_algorithm()` and `reconcile_lb_type()`: Update LB settings when changed
- `cleanup()`: Removes all services and targets before deleting the load balancer

**finalizers.rs**: Prevents accidental service deletion
- Adds `robotlb/finalizer` to services to ensure cleanup happens before deletion
- Cleanup triggered when `deletion_timestamp` is set on the service

**label_filter.rs**: Node label filtering
- Implements custom label selector syntax for the `robotlb/node-selector` annotation
- Supports: `key=value`, `key!=value`, `key` (exists), `!key` (does not exist)

**config.rs**: Operator configuration via CLI args and environment variables
- All configs prefixed with `ROBOTLB_`
- Contains defaults for LB settings (location, type, algorithm, healthcheck params)

**consts.rs**: Annotation and constant definitions
- All service annotations use `robotlb/` prefix
- Gateway class name: `robotlb`

**gateway.rs**: Gateway API load balancer management
- `GatewayLoadBalancer` wraps `LoadBalancer` for Gateway resources
- `try_from_gateway()`: Constructs from Gateway spec and annotations
- Reuses core Hetzner LB provisioning logic from `lb.rs`

**routes.rs**: HTTPRoute and TCPRoute handling
- `extract_http_route_backends()`: Extracts backend Services from HTTPRoute
- `extract_tcp_route_backends()`: Extracts backend Services from TCPRoute
- `get_backend_services()`: Fetches actual Service resources from K8s
- `determine_port_mappings()`: Maps Gateway listener ports to backend ports

### Key Behaviors

1. **Node Selection**: Two modes controlled by `--dynamic-node-selector`:
   - Dynamic (default): Finds nodes by looking up where pods matching service selector are running
   - Static: Uses `robotlb/node-selector` annotation to filter nodes

2. **Network Handling**:
   - If `robotlb/lb-network` annotation is set, uses private network (InternalIP)
   - Otherwise uses public IPs (ExternalIP)
   - Supports optional `robotlb/lb-private-ip` for specific IP allocation

3. **Service Filtering**: Only processes services with:
   - `type: LoadBalancer`
   - `loadBalancerClass: robotlb` (or no class specified, defaults to `robotlb`)

4. **Protocol Support**: Only TCP protocol is currently supported; UDP ports are ignored with warnings

5. **Gateway API Support**:
   - Supports Gateway resources with `gatewayClassName: robotlb`
   - Extracts listeners from `Gateway.spec.listeners` for port configuration
   - Supports both HTTPRoute and TCPRoute resources
   - HTTPRoute is in standard API (`gateway_api::apis::standard::httproutes`)
   - TCPRoute is in experimental API (`gateway_api::apis::experimental::tcproutes`)
   - Routes must reference the Gateway via `spec.parentRefs`
   - Backend Services are discovered from route `backendRefs`
   - Node discovery uses same logic as Service controller (dynamic or selector-based)
   - Updates `Gateway.status.addresses` with provisioned load balancer IPs

## Building and Development

### Build Commands
```bash
# Build debug version
cargo build

# Build optimized release version
cargo build --release

# Run locally (requires .env file with ROBOTLB_HCLOUD_TOKEN)
cargo run -- --hcloud-token <token>
```

### Testing
There are no unit tests in the repository currently. Manual testing requires:
- A Kubernetes cluster with access to the API
- Hetzner Cloud API token
- Hetzner Robot bare-metal servers configured with vSwitch

### Code Style
- Uses strict Clippy lints (all, pedantic, nursery)
- Allows `module_name_repetitions` and `missing_errors_doc`
- Formatted with rustfmt (config in `.rustfmt.toml`)

### Dependencies
- `kube`: Kubernetes client library (v2.0.1)
- `k8s-openapi`: Kubernetes API types v0.26.0 (v1.31 feature)
- `gateway-api`: Gateway API CRD types (v0.19.0, supports Gateway API v1.4.0)
- `hcloud`: Hetzner Cloud API client
- `tokio`: Async runtime
- `clap`: CLI argument parsing with environment variable support
- `tracing`: Structured logging

### Performance
- Uses jemalloc allocator for better memory performance (non-MSVC targets)
- Release profile optimized with LTO and single codegen unit

## Gateway API Architecture

### Resource Flow
```
Gateway (with gatewayClassName: robotlb)
  └─ spec.listeners[] → defines ports and protocols
  └─ Annotations (robotlb/*) → LB configuration

HTTPRoute / TCPRoute
  └─ spec.parentRefs[] → references Gateway
  └─ spec.rules[].backendRefs[] → references Services

Services (backends)
  └─ Pods → determine target nodes
  └─ Nodes → IP addresses added as LB targets
```

### Reconciliation Flow (Gateway)
1. Gateway controller triggered on Gateway/Route changes
2. Validate `gatewayClassName == "robotlb"`
3. Parse Gateway annotations for LB configuration (same as Service annotations)
4. Extract listener ports from `spec.listeners`
5. Find all HTTPRoute/TCPRoute resources referencing this Gateway
6. Extract backend Services from routes
7. Determine target nodes using pod discovery or node selectors
8. Create/update Hetzner load balancer
9. Update `Gateway.status.addresses` with LB IPs

### Important Type Details
- Gateway types from `gateway_api::apis::standard::gateways`
- HTTPRoute from `gateway_api::apis::standard::httproutes::HTTPRoute`
- TCPRoute from `gateway_api::apis::experimental::tcproutes::TCPRoute`
- Spec fields are **not** `Option` (direct access: `gateway.spec.listeners`)
- HTTPRoute `backend_refs` is `Option<Vec<...>>`
- TCPRoute `backend_refs` is `Vec<...>` (not Option)
- HTTPRoute `rules` is `Option<Vec<...>>`
- TCPRoute `rules` is `Vec<...>` (not Option)

## Common Patterns

### Adding New Service Annotations
1. Add constant to `src/consts.rs`
2. Parse annotation in `LoadBalancer::try_from_svc()` in `src/lb.rs`
3. Add field to `LoadBalancer` struct
4. Use field in relevant reconciliation method

### Adding New Operator Config Options
1. Add field to `OperatorConfig` in `src/config.rs` with `#[arg]` attribute
2. Set env variable name with `ROBOTLB_` prefix
3. Use in reconciliation logic via `context.config`

### Adding Gateway-Related Features
1. Gateway configuration parsing happens in `gateway::GatewayLoadBalancer::try_from_gateway()`
2. Annotations are reused from Service controller (same constants from `consts.rs`)
3. Route backend extraction in `routes.rs`
4. Gateway finalizer functions in `finalizers.rs` (separate from Service finalizers)

### Error Handling
- Custom error type `RobotLBError` in `src/error.rs`
- `RobotLBError::SkipService` is special: silently skips service without error log
- `RobotLBError::SkipGateway` is special: silently skips gateway without error log
- All other errors trigger requeue with 30s delay
- Use `?` operator for error propagation; controller handles logging

## Deployment

Deployed via Helm chart at `oci://ghcr.io/intreecom/charts/robotlb`. See README.md for deployment instructions.
