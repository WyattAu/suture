---
document_id: YP-DIST-CONSENSUS-001
version: 1.0.0
status: DRAFT
domain: Distributed Systems
created: 2026-03-27
author: DeepThought
confidence_level: 0.80
tqa_level: 3
---

# YP-DIST-CONSENSUS-001: Distributed Consensus for Suture Hub

## 1. Executive Summary

Suture Hub (v0.4+) introduces multi-node coordination for collaborative editing sessions.
When multiple editors interact with the same project simultaneously, a consensus protocol
ensures that all nodes agree on the order of patch application, lease ownership, and CAS
state.

This Yellow Paper specifies the use of the **Raft consensus algorithm** for Suture Hub's
distributed coordination layer. Raft was chosen for its understandability, proven correctness,
and strong leadership model — which aligns naturally with Suture's client-server architecture.

**Scope:**
- Leader election and term management.
- Log replication for patch ordering.
- Lease lifecycle for exclusive edit sessions.
- Network partition handling and recovery.

**Out of Scope:**
- Detailed Raft implementation (deferred to implementation specs).
- CAS replication strategy (log-structured vs. active-active).
- Authentication and authorization (→ YP-SEC-001).

---

## 2. Key Concepts

### 2.1 Raft Overview

Raft decomposes consensus into three independent subproblems:

1. **Leader Election:** A new leader is chosen when the existing leader fails.
2. **Log Replication:** The leader accepts patch submissions from clients and replicates
   them across all followers, ensuring total order.
3. **Safety:** If any server has applied a patch to its state machine, no other server
   may apply a different patch at the same log index.

### 2.2 Roles

| Role | Responsibility | Count |
|------|---------------|-------|
| **Leader** | Accepts all client requests, replicates log entries to followers | Exactly 1 per term |
| **Follower** | Responds to leader RPCs, redirects client requests to leader | All non-leader nodes |
| **Candidate** | Transient role during leader election, solicits votes | Self-promoted nodes |

### 2.3 Terms

Raft divides time into **terms** of arbitrary length, numbered monotonically:

$$t_0 < t_1 < t_2 < \cdots$$

Each term begins with a leader election. If a leader is elected, it serves for the
remainder of the term. If the election times out (split vote), a new term begins immediately.

---

## 3. Leader Election

### 3.1 Election Trigger

A follower becomes a candidate and starts an election when it does not receive heartbeat
AppendEntries RPCs from the current leader within the **election timeout**:

$$T_{\text{election}} \in [T_{\min}, T_{\max}]$$

where the timeout is randomized per node to prevent split votes. For Suture Hub:

$$T_{\min} = 150\text{ms}, \quad T_{\max} = 300\text{ms}$$

### 3.2 Election Algorithm

```
Election (simplified)
=====================

1:  On election timeout:
2:    Increment current_term
3:    Transition to Candidate
4:    Vote for self
5:    Reset election timer
6:    Send RequestVote RPCs to all other nodes
7:
8:  If votes received > majority (N/2 + 1):
9:    Become Leader
10:   Begin sending AppendEntries heartbeats
11:
12: If AppendEntries received from leader with term >= current_term:
13:   Revert to Follower
```

### 3.3 Safety Invariant

> *At most one leader can be elected per term (Election Safety).*

*Proof.* Each voter grants at most one vote per term. A candidate needs a majority of votes
to become leader. Since majorities of a fixed set overlap, at most one candidate can receive
a majority in any given term. ∎

---

## 4. Log Replication

### 4.1 Patch Submission

When a client submits a patch to the leader:

1. Leader appends the patch to its log as an uncommitted entry.
2. Leader sends AppendEntries RPC to all followers, carrying the new log entry.
3. Follower appends the entry to their log and acknowledges.
4. Once a majority of followers acknowledge, the leader **commits** the entry.
5. Leader notifies followers of the commit in the next AppendEntries heartbeat.
6. Followers apply the committed patch to their state machines.

### 4.2 Consistency Guarantee

> *If two log entries have the same index and term, they store the same patch.*

This follows from the leader's log-matching property: a new leader forces conflicting
followers to overwrite their logs with the leader's log, ensuring convergence.

### 4.3 Log Entry Format

```
LogEntry = {
    term:        u64,
    index:       u64,
    patch_hash:  [u8; 32],    // BLAKE3 address of the patch blob in CAS
    timestamp:   u64,          // Monotonic nanoseconds
}
```

The patch payload is not stored in the Raft log — only its CAS content address. This keeps
the Raft log compact and leverages the CAS for deduplication.

---

## 5. Lease Management

### 5.1 Definition

A **lease** grants a specific node exclusive rights to modify a set of addresses in the
project state. Leases prevent conflicting concurrent edits that would require conflict
resolution.

$$\text{Lease} = (\text{holder},\ \text{addresses},\ T_{\text{acquire}},\ T_{\text{expiry}})$$

### 5.2 Lease Lifecycle

```
ALG-LEASE-001: Lease Management
================================

Acquire:
1:  Client requests lease on address set A
2:  Leader checks: no existing lease overlaps with A
3:  Leader appends LeaseGrant(A, holder, T_expiry) to Raft log
4:  On commit, leader grants lease and responds to client

Heartbeat:
5:  Client sends LeaseHeartbeat before T_expiry
6:  Leader extends T_expiry by T_lease_duration
7:  Leader appends LeaseExtend to Raft log

Expire:
8:  Leader's background timer detects T_now > T_expiry
9:  Leader appends LeaseRevoke to Raft log
10: Address set A becomes available for other clients
```

### 5.3 Lease Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| $T_{\text{lease\_duration}}$ | 30 s | Balance between responsiveness and overhead |
| $T_{\text{heartbeat\_interval}}$ | 10 s | 3 heartbeats per lease period (redundancy) |
| $T_{\text{grace\_period}}$ | 5 s | Allows network-delayed heartbeats to arrive |

---

## 6. Network Partition Handling

### 6.1 Partition Detection

When a network partition isolates a subset of nodes:

- The **majority partition** retains the leader and continues normal operation.
- The **minority partition** times out on leader heartbeats, triggers a new election,
  but cannot achieve majority — no new leader is elected.
- Minority partition nodes transition to a **read-only degraded mode**, serving cached
  state but rejecting patch submissions.

### 6.2 Partition Recovery

When the partition heals:

1. Rejoining nodes receive AppendEntries from the leader.
2. The leader's log takes precedence — conflicting entries on rejoining nodes are
   overwritten (Raft log-matching property).
3. Rejoining nodes replay any committed entries they missed.
4. Once logs are synchronized, nodes resume full participation.

### 6.3 Client Behavior

Clients connected to the minority partition are informed of degraded state via lease
timeout. They must reconnect to the majority partition before submitting patches.

---

## 7. Integration with Suture Architecture

| Component | Raft Responsibility |
|-----------|-------------------|
| Patch ordering | Log replication ensures total order of patch application |
| CAS dedup | Content addresses in log entries reference CAS blobs; CAS consistency follows from log consistency |
| Lease management | Lease grant/revoke/extend are Raft log entries |
| Hub Web UI | Reads from leader's committed state; writes go through leader |
| VFS mounts | Follow local committed state; conflict with remote resolved by leader |

---

## 8. Bibliography

1. **Ongaro, Diego, and John Ousterhout.** "In Search of an Understandable Consensus
   Algorithm." *USENIX ATC '14*. 2014.
   *https://raft.github.io/raft.pdf*. The foundational Raft paper. All consensus behavior
   in Suture Hub derives from this specification.

2. **Howard, Heidi, et al.** "Flexible paxos: Quorum intersection revisited."
   *Advances in Distributed Computing (OPODIS '16)*. Informs Suture's approach to
   membership changes and dynamic quorum sizing.

3. **Chandra, Tushar D., and Sam Toueg.** "Unreliable failure detectors for reliable
   distributed systems." *Journal of the ACM* 43.2 (1996). Provides the theoretical
   foundation for the failure detector underlying Raft's election timeout.

---

## 9. Revision History

| Version | Date | Author | Description |
|---------|------|--------|-------------|
| 1.0.0 | 2026-03-27 | DeepThought | Initial draft. Raft consensus, lease management, partition handling. |

---

*End of YP-DIST-CONSENSUS-001*
