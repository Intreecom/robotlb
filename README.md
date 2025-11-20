# Hetzner LoadBalancer for bare-metal robot clusters

This project is useful when you've deployed a bare-metal Kubernetes cluster on Hetzner Robot and want to use Hetzner's cloud load balancer.

This operator integrates them together, supporting two APIs:
1. **LoadBalancer Services**: Traditional Kubernetes Service resources of type `LoadBalancer`
2. **Gateway API**: Modern Kubernetes Gateway API (v1) for advanced routing capabilities

You can follow the [TUTORIAL.md](./tutorial.md) to see how to set up a cluster using RobotLB from scratch.

## Prerequisites

Before using this operator, make sure:

1. You have a cluster deployed on [Hetzner robot](https://robot.hetzner.com/) (at least agent nodes);
2. You've created a [vSwitch](https://docs.hetzner.com/robot/dedicated-server/network/vswitch/) for these servers;
3. You've assigned IPs to your dedicated servers within the vSwitch network.
4. You have a cloud network with subnet that points to the vSwitch ([Tutorial](https://docs.hetzner.com/cloud/networks/connect-dedi-vswitch/));
5. You’ve specified node IPs using the `--node-ip` argument with the private IP.

If you meet all the requirements, you can deploy `robotlb`.

## Deploying

The recommended way to deploy this operator is using the Helm chart.

```bash
helm show values oci://ghcr.io/intreecom/charts/robotlb > values.yaml
# Edit values.yaml to suit your needs
# Set `envs.ROBOTLB_HCLOUD_TOKEN`.
helm install robotlb oci://ghcr.io/intreecom/charts/robotlb -f values.yaml
```

After the chart is installed, you should be able to create `LoadBalancer` services and Gateway API resources.

## How it works

The operator runs two concurrent controllers:
1. **Service Controller**: Listens for services of type `LoadBalancer` and creates Hetzner load balancers
2. **Gateway Controller**: Listens for Gateway API resources (Gateway, HTTPRoute, TCPRoute) and provisions load balancers

Nodes are selected based on where the target pods are deployed, which is determined by searching for pods with the service's selector. This behavior can be configured.


## Configuration

This project has two places for configuration: environment variables and resource annotations (Service or Gateway).

### Envs

Environment variables are mainly used to override default arguments and provide sensitive information.

Here’s a complete list of parameters for the operator's binary:

```
Usage: robotlb [OPTIONS] --hcloud-token <HCLOUD_TOKEN>

Options:
  -t, --hcloud-token <HCLOUD_TOKEN>
          `HCloud` API token [env: ROBOTLB_HCLOUD_TOKEN=]
      --default-network <DEFAULT_NETWORK>
          Default network to use for load balancers. If not set, then only network from the service annotation will be used [env: ROBOTLB_DEFAULT_NETWORK=]
      --dynamic-node-selector
          If enabled, the operator will try to find target nodes based on where the target pods are actually deployed. If disabled, the operator will try to find target nodes based on the node selector [env: ROBOTLB_DYNAMIC_NODE_SELECTOR=]
      --default-lb-retries <DEFAULT_LB_RETRIES>
          Default load balancer healthcheck retries cound [env: ROBOTLB_DEFAULT_LB_RETRIES=] [default: 3]
      --default-lb-timeout <DEFAULT_LB_TIMEOUT>
          Default load balancer healthcheck timeout [env: ROBOTLB_DEFAULT_LB_TIMEOUT=] [default: 10]
      --default-lb-interval <DEFAULT_LB_INTERVAL>
          Default load balancer healhcheck interval [env: ROBOTLB_DEFAULT_LB_INTERVAL=] [default: 15]
      --default-lb-location <DEFAULT_LB_LOCATION>
          Default location of a load balancer. https://docs.hetzner.com/cloud/general/locations/ [env: ROBOTLB_DEFAULT_LB_LOCATION=] [default: hel1]
      --default-balancer-type <DEFAULT_BALANCER_TYPE>
          Type of a load balancer. It differs in price, number of connections, target servers, etc. The default value is the smallest balancer. https://docs.hetzner.com/cloud/load-balancers/overview#pricing [env: ROBOTLB_DEFAULT_LB_TYPE=] [default: lb11]
      --default-lb-algorithm <DEFAULT_LB_ALGORITHM>
          Default load balancer algorithm. Possible values: * `least-connections` * `round-robin` https://docs.hetzner.com/cloud/load-balancers/overview#load-balancers [env: ROBOTLB_DEFAULT_LB_ALGORITHM=] [default: least-connections]
      --default-lb-proxy-mode-enabled
          Default load balancer proxy mode. If enabled, the load balancer will act as a proxy for the target servers. The default value is `false`. https://docs.hetzner.com/cloud/load-balancers/faq/#what-does-proxy-protocol-mean-and-should-i-enable-it [env: ROBOTLB_DEFAULT_LB_PROXY_MODE_ENABLED=]
      --ipv6-ingress
          Whether to enable IPv6 ingress for the load balancer. If enabled, the load balancer's IPv6 will be attached to the service as an external IP along with IPv4 [env: ROBOTLB_IPV6_INGRESS=]
      --log-level <LOG_LEVEL>
          [env: ROBOTLB_LOG_LEVEL=] [default: INFO]
  -h, --help
          Print help
```


### Service annotations


```yaml
apiVersion: v1
kind: Service
metadata:
  name: target
  annotations:
    # Custom name of the balancer to create on Hetzner. Defaults to service name.
    robotlb/balancer: "custom name"
    # Hetzner cloud network. If this annotation is missing, the operator will try to
    # assign external IPs to the load balancer if available. Otherwise, the update won't happen.
    robotlb/lb-network: "my-net"
    # Requests specific IP address for the load balancer in the private network. If not specified,
    # a random one is given. This parameter does nothing in case if network is not specified.
    robotlb/lb-private-ip: "10.10.10.10"
    # Node selector for the loadbalancer. This is only required if ROBOTLB_DYNAMIC_NODE_SELECTOR
    # is set to false. If not specified then, all nodes will be selected as LB targets by default.
    # This property helps you filter out nodes.
    # Filters are separated by commas and should have one of the following formats:
    # * key=value  -- checks that the node has a label `key` with value `value`;
    # * key!=value -- verifies that key either doesn't exist or isn't equal to `value`;
    # * !key       -- verifies that the node doesn't have a label `key`;
    # * key        -- verifies that the node has a label `key`.
    robotlb/node-selector: "node-role.kubernetes.io/control-plane!=true,beta.kubernetes.io/arch=amd64"
    ### Load balancer healthcheck options. ###
    # How often to run health probes.
    robotlb/lb-check-interval: "5"
    # Timeout for a single probe.
    robotlb/lb-timeout: "3"
    # How many failed probes before marking the node as unhealthy.
    robotlb/lb-retries: "3"

    ### Load balancer options ###
    # Whether to use proxy mode for this target.
    # https://docs.hetzner.com/cloud/load-balancers/faq/#what-does-proxy-protocol-mean-and-should-i-enable-it
    robotlb/lb-proxy-mode: "false"
    # Location of the load balancer. This expects the code of one of Hetzner's available locations.
    robotlb/lb-location: "hel1"
    # Balancing algorithm. Can be either
    # * least-connection
    # * round-robin
    robotlb/lb-algorithm: "least-connection"
    # Type of balancer.
    robotlb/balancer-type: "lb11"
spec:
  type: LoadBalancer
  # If dynamic node selector is enabled, nodes will be found
  # using this property.
  selector:
    app: target
  ports:
    # Currently only TCP protocol is supported. UDP will be ignored.
    - protocol: TCP
      port: 80  # This will become the listening port on the LB
      targetPort: 80
```

## Gateway API Support

RobotLB supports the Kubernetes Gateway API as a modern alternative to LoadBalancer Services. The Gateway API provides more advanced routing capabilities and a role-oriented design.

### Prerequisites

Install the Gateway API CRDs in your cluster:

```bash
kubectl apply -f https://github.com/kubernetes-sigs/gateway-api/releases/download/v1.2.0/standard-install.yaml
```

For TCPRoute support, install the experimental channel:

```bash
kubectl apply -f https://github.com/kubernetes-sigs/gateway-api/releases/download/v1.2.0/experimental-install.yaml
```

### Gateway Configuration

Gateway resources use the same `robotlb/*` annotations as Services:

```yaml
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: example-gateway
  annotations:
    # All the same annotations as Services are supported
    robotlb/lb-network: "my-net"
    robotlb/lb-location: "hel1"
    robotlb/balancer-type: "lb11"
    robotlb/lb-algorithm: "least-connections"
    robotlb/lb-check-interval: "5"
    robotlb/lb-timeout: "3"
    robotlb/lb-retries: "3"
spec:
  gatewayClassName: robotlb
  listeners:
  - name: http
    protocol: HTTP
    port: 80
  - name: https
    protocol: HTTPS
    port: 443
```

### HTTPRoute Example

```yaml
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: example-route
spec:
  parentRefs:
  - name: example-gateway
  rules:
  - matches:
    - path:
        type: PathPrefix
        value: /app
    backendRefs:
    - name: my-service
      port: 8080
```

### TCPRoute Example

```yaml
apiVersion: gateway.networking.k8s.io/v1alpha2
kind: TCPRoute
metadata:
  name: example-tcp-route
spec:
  parentRefs:
  - name: example-gateway
  rules:
  - backendRefs:
    - name: my-tcp-service
      port: 3306
```

### How Gateway API Works with RobotLB

1. Create a Gateway resource with `gatewayClassName: robotlb`
2. The Gateway controller provisions a Hetzner Cloud load balancer
3. Create HTTPRoute or TCPRoute resources that reference the Gateway
4. RobotLB discovers backend Services from the routes
5. Target nodes are determined by finding where backend Service pods run
6. The load balancer is configured with the appropriate ports and targets
7. Gateway status is updated with the load balancer's IP addresses

### Benefits of Gateway API

- **Rich Routing**: Advanced HTTP routing with header matching, path rewrites, etc.
- **Role-Oriented**: Separation between infrastructure (Gateway) and routing (Routes)
- **Protocol Support**: Native support for HTTP, HTTPS, TCP, and more
- **Portable**: Works the same across different implementations
- **Type-Safe**: Strongly typed API without relying on annotations for core functionality

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Intreecom/robotlb&type=Date)](https://star-history.com/#Intreecom/robotlb&Date)
