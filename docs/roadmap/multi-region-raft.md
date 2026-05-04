# Multi-Region Raft Clusters

## Design

Enable Suture Hub to run as a multi-region Raft cluster for global availability
and low-latency reads.

### Architecture

```
                  +----------+
                  |  Region  |
                  |  (Leader)|
                  +----+-----+
                       |
          +------------+------------+
          |            |            |
    +-----+---+  +----+----+  +-----+---+
    | Region 2 |  | Region 3 |  | Region 4 |
    | (Follower)|  | (Follower)|  | (Follower)|
    +----------+  +----------+  +----------+
```

### Raft Protocol Enhancements

1. **Multi-region transport**: Replace loopback gRPC with TLS-secured
   inter-region communication
2. **Learner promotion**: New regions start as learners, catch up via
   snapshot transfer, then become voters
3. **Leader election with region awareness**: Prefer leader in region
   with lowest latency to majority of clients
4. **Read delegation**: Followers can serve reads with configurable
   consistency level (strong, bounded-staleness, eventual)

### Configuration

```toml
[raft]
cluster_id = "suture-prod"
regions = ["iad", "lhr", "syd"]
region = "iad"  # this node's region
peers = [
    { id = 1, region = "iad", addr = "suture-iad.internal:7000" },
    { id = 2, region = "lhr", addr = "suture-lhr.internal:7000" },
    { id = 3, region = "syd", addr = "suture-syd.internal:7000" },
]
snapshot_interval = 10000
replication_factor = 2
```

### Implementation Phases

**Phase 1: Single-region HA** (current)
- Multiple nodes in one region
- Leader election, log replication, snapshots
- Status: COMPLETE

**Phase 2: Multi-region read replicas**
- Add follower-only nodes in other regions
- Read delegation with bounded staleness
- Async snapshot transfer to new regions

**Phase 3: Multi-region voting**
- Full voting members in multiple regions
- TLS-secured inter-region gRPC
- Region-aware leader election
