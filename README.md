# Hetzner LoadBalancer for bare-metal robot clusters

This project is useful when you've deployed a bare-metal Kubernetes cluster on Hetzner Robot and
want to use Hetzner's cloud load balancer.

This small operator integrates them together, allowing you to use the `LoadBalancer` service type.

You can follow the [TUTORIAL.md](./tutorial.md) to see how to set up a cluster using RobotLB from
scratch.

## Prerequisites

Before using this operator, make sure:

1. You have a cluster deployed on [Hetzner robot](https://robot.hetzner.com/) (at least agent
   nodes);
2. You've created a [vSwitch](https://docs.hetzner.com/robot/dedicated-server/network/vswitch/) for
   these servers;
3. You've assigned IPs to your dedicated servers within the vSwitch network.
4. You have a cloud network with subnet that points to the vSwitch
   ([Tutorial](https://docs.hetzner.com/cloud/networks/connect-dedi-vswitch/));
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

After the chart is installed, you should be able to create `LoadBalancer` services.

### High availability

`robotlb` supports safe multi-replica deployment via Kubernetes Lease-based leader election.
Run at least 2 replicas and use pod anti-affinity so only one pod is active leader while the
other is standby.

Example Helm values:

```yaml
replicaCount: 2

affinity:
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchLabels:
            app.kubernetes.io/name: robotlb
            app.kubernetes.io/instance: robotlb
        topologyKey: kubernetes.io/hostname
```

## How it works

The operator listens to the Kubernetes API for services of type `LoadBalancer` and creates Hetzner
load balancers that point to nodes based on `node-ip`.

Nodes are selected based on where the service's target pods are deployed, which is determined by
searching for pods with the service's selector. This behavior can be configured.

It also watches `EndpointSlice` objects to react quickly to backend rollout/placement changes, with
an additional periodic reconcile as a resilience fallback.

## Configuration

This project has two places for configuration: environment variables and service annotations.

### Envs

Environment variables are mainly used to override default arguments and provide sensitive
information.

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
      --leader-election-namespace <LEADER_ELECTION_NAMESPACE>
          Optional namespace for the leader election lease. If not set, robotlb auto-detects runtime namespace [env: ROBOTLB_LEADER_ELECTION_NAMESPACE=]
      --leader-election-lease-name <LEADER_ELECTION_LEASE_NAME>
          Name of the Kubernetes Lease object used for leader election [env: ROBOTLB_LEADER_ELECTION_LEASE_NAME=] [default: robotlb-leader-election]
      --leader-election-lease-ttl-secs <LEADER_ELECTION_LEASE_TTL_SECS>
          Lease TTL in seconds [env: ROBOTLB_LEADER_ELECTION_LEASE_TTL_SECS=] [default: 15]
      --leader-election-renew-interval-secs <LEADER_ELECTION_RENEW_INTERVAL_SECS>
          How often to attempt lease acquire/renew in seconds [env: ROBOTLB_LEADER_ELECTION_RENEW_INTERVAL_SECS=] [default: 5]
      --log-level <LOG_LEVEL>
          [env: ROBOTLB_LOG_LEVEL=] [default: INFO]
  -h, --help
          Print help
```

When deploying without the Helm chart, ensure RBAC includes:

- `discovery.k8s.io/endpointslices`: `get,list,watch`
- `coordination.k8s.io/leases`: `get,list,watch,create,patch,update`

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
      port: 80 # This will become the listening port on the LB
      targetPort: 80
```

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=forkline/robotlb&type=Date)](https://star-history.com/#forkline/robotlb&Date)
