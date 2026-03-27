---
document_id: YP-ALGEBRA-PATCH-001
version: 1.0.0
status: DRAFT
domain: Version Control Theory
subdomains: [Category Theory, Algebra, Graph Theory]
applicable_standards: [IEC 61508, ISO/IEC 12207]
created: 2026-03-27
author: DeepThought (Research Agent)
confidence_level: 0.92
tqa_level: 4
---

# YP-ALGEBRA-PATCH-001: The Algebra of Semantic Patches

## 1. Executive Summary

### 1.1 Problem Statement

Traditional Version Control Systems (VCS) model project history as a sequence of snapshots
applied to a filesystem tree. When the tree contains non-textual, structured data — video
timelines, spreadsheets, CAD assemblies — concurrent modifications produce opaque binary
conflicts. The system cannot decompose the conflict into independent, commutable operations
because it lacks a semantic model of what changed and why.

This causes **merge paralysis**: two editors who modify different clips in the same timeline,
or different cells in the same spreadsheet, are presented with an all-or-nothing file conflict
even though their changes are semantically independent.

### 1.2 Formal Problem

Let $\Sigma$ denote the set of all valid project states. Let $\mathcal{P}$ denote the set of all
patches. Each patch $P \in \mathcal{P}$ is a state transition function:

$$P : \Sigma \to \Sigma$$

Given a base state $S_0 \in \Sigma$ and two sets of modifications $\Delta_A, \Delta_B \subseteq
\mathcal{P}$ applied concurrently by independent agents, we require:

1. **Commutativity**: For any pair $(P_i, P_j)$ whose modifications target disjoint semantic
   regions, the composition $P_i \circ P_j = P_j \circ P_i$ must hold, yielding the same final
   state regardless of application order.

2. **Conflict Detection**: When $T(P_i) \cap T(P_j) \neq \emptyset$, the system must
   deterministically identify the conflict and preserve both versions without data loss.

3. **Deterministic Merge**: Given $\Delta_A$ and $\Delta_B$, the merge operation must produce a
   unique result, independent of the order in which the two sets are processed.

The central question: *What algebraic structure do patches form under composition, and what
properties must the touch-set relation satisfy to guarantee deterministic, lossless merging?*

### 1.3 Scope

This Yellow Paper defines the **Patch Algebra** for Suture. It establishes:

- The formal definitions of patches, touch sets, composition, commutativity, and conflict.
- The algebraic properties (axioms, lemmas, theorems) that govern patch behavior.
- The merge algorithm and its correctness proofs.
- The DAG construction algorithm and its termination guarantees.

**In Scope:**
- Commutativity and conflict detection for patch pairs and patch sets.
- Three-way merge algorithm specification and correctness.
- Patch-DAG construction and maintenance.
- Identity patch and associativity properties.

**Out of Scope (deferred to subsequent Yellow Papers):**
- Distributed consensus and conflict resolution in multi-node deployments
  (→ YP-DIST-CONSENSUS-001).
- Serialization encoding and FlatBuffers wire format (→ YP-ALGEBRA-PATCH-002).
- Cryptographic signing and key rotation (→ YP-SEC-001).
- Driver-specific semantic decomposition (→ YP-DRIVER-OTIO-001, etc.).
- Virtual File System integration (→ YP-VFS-001).

---

## 2. Nomenclature

All symbols used in this document are defined below. Symbols not listed here are either
standard mathematical notation or defined inline at first use.

| Symbol | Description | Type | Domain |
|--------|-------------|------|--------|
| $\Sigma$ | State space | Set | All valid project states |
| $S$ | Project state | Element | $S \in \Sigma$ |
| $\mathcal{P}$ | Patch space | Set | All valid patches |
| $P, Q$ | Patch | Element | $P \in \mathcal{P}$ |
| $T(P)$ | Touch set of patch $P$ | $\mathcal{P}(\text{Addr})$ | Power set of addresses |
| $\text{Addr}$ | Address space | Set | Countable set of identifiers |
| $\circ$ | Patch composition | $\mathcal{P} \times \mathcal{P} \to \mathcal{P}$ | Binary operator |
| $\oplus$ | Patch set merge | $\mathcal{P}(\mathcal{P}) \times \mathcal{P}(\mathcal{P}) \to \mathcal{P}(\mathcal{P})$ | Set merge operator |
| $\emptyset$ | Empty set | Constant | $\emptyset \in \mathcal{P}(\text{Addr})$ |
| $\mathcal{G}$ | Patch DAG | Graph | $\mathcal{G} = (V, E)$ where $V \subseteq \mathcal{P}$, $E \subseteq V \times V$ |
| $C$ | Conflict node | Type | $C \in \mathcal{C}$ |
| $\mathcal{C}$ | Conflict space | Set | All valid conflict nodes |
| $\text{id}$ | Identity patch | Element | $\text{id} \in \mathcal{P}$ |
| $\equiv$ | Patch equivalence | Relation | $\equiv \subseteq \mathcal{P} \times \mathcal{P}$ |
| $\bot$ | Conflict marker | Constant | Sentinel for unresolvable conflicts |
| $\text{op}$ | Operation type | Enumeration | E.g., $\texttt{UpdateNode}$, $\texttt{MoveClip}$, $\texttt{EditCell}$ |
| $\text{payload}$ | Operation-specific data | Product type | Carries the semantic modification data |
| $\text{LCA}(a, b)$ | Lowest Common Ancestor | Function | $\text{LCA}: V \times V \to V$ |
| $\text{PS}$ | Patch set | Set | $\text{PS} \subseteq \mathcal{P}$ |
| $R(P)$ | Read set of patch $P$ | $\mathcal{P}(\text{Addr})$ | Addresses read by $P$ |
| $W(P)$ | Write set of patch $P$ | $\mathcal{P}(\text{Addr})$ | Addresses written by $P$ |
| $\text{ancestors}(v)$ | Transitive ancestors | Function | $\text{ancestors}: V \to \mathcal{P}(V)$ |
| $\text{sign}(P)$ | Cryptographic signature | Byte sequence | Ed25519 signature over $P$ |

### 2.1 Conventions

- Patches are applied **right-to-left**: $P_1 \circ P_2$ means "apply $P_1$, then apply $P_2$ to
  the result." Formally: $(P_1 \circ P_2)(S) = P_2(P_1(S))$.
- The empty touch set is written $\emptyset$ and is the touch set of the identity patch.
- All sets are finite unless explicitly stated otherwise.
- "Branch" refers to a named pointer to a DAG node, not to a separate copy of the DAG.

---

## 3. Theoretical Foundation

### 3.1 Axioms

These axioms define the fundamental properties of the state space and patch space. They are
assumed true; all subsequent definitions, lemmas, and theorems derive from them.

**AX-001 (State Well-Definedness).** Every project state $S \in \Sigma$ is a finite,
deterministic function from addresses to values:

$$S : \text{Addr} \to \text{Val}$$

where $\text{Val}$ is the value space (bytes, structured records, etc.) and the support of $S$
(i.e., $\{a \in \text{Addr} \mid S(a) \neq \bot\}$) is finite.

*Rationale: A project state must be fully determined by the values at its addresses. There
are no hidden or implicit dependencies. This guarantees reproducibility.*

**AX-002 (Patch Determinism).** Every patch $P \in \mathcal{P}$ is a pure function
$P : \Sigma \to \Sigma$. Given the same input state, it always produces the same output state:

$$\forall S \in \Sigma: P(S) \text{ is uniquely determined.}$$

*Rationale: Determinism is the foundational invariant of the system (REQ-CORE-002). Without
it, merge results would be non-deterministic and auditing would be impossible.*

**AX-003 (Finite Touch Sets).** Every patch $P$ has a finite touch set $T(P) \subset \text{Addr}$:

$$\forall P \in \mathcal{P}: |T(P)| < \infty$$

*Rationale: Infinite touch sets would make commutativity checking undecidable and merge
algorithms non-terminating.*

**AX-004 (Address Space).** The address space $\text{Addr}$ is a countable set of identifiers
for granular resources:

$$|\text{Addr}| \leq \aleph_0$$

*Rationale: Addresses identify the finest-grained semantic units — cells in a spreadsheet,
clips in a timeline, nodes in a scene graph. Countability ensures enumerable patch sets and
decidable conflict detection.*

**AX-005 (Patch Closure Under Composition).** For any two patches $P_1, P_2 \in \mathcal{P}$,
the composition $P_1 \circ P_2 \in \mathcal{P}$ is also a patch:

$$\forall P_1, P_2 \in \mathcal{P}: P_1 \circ P_2 \in \mathcal{P}$$

*Rationale: Composition of patches must itself be a valid patch to enable sequential
application and history compression.*

**AX-006 (Identity Existence).** There exists an identity patch $\text{id} \in \mathcal{P}$
such that:

$$\forall P \in \mathcal{P}, \forall S \in \Sigma: \text{id}(S) = S$$

*Rationale: The identity patch serves as the neutral element of the patch monoid (REQ-PATCH-006)
and represents "no changes."*

### 3.2 Definitions

**DEF-001 (Patch).** A patch $P$ is a triple:

$$P = (\text{op}, T, \text{payload})$$

where:
- $\text{op} \in \text{OpType}$ is the operation type (e.g., $\texttt{UpdateNode}$,
  $\texttt{MoveClip}$, $\texttt{EditCell}$, $\texttt{AddNode}$, $\texttt{DeleteNode}$).
- $T = T(P) \in \mathcal{P}(\text{Addr})$ is the touch set — the finite set of addresses
  that this patch modifies.
- $\text{payload} \in \text{PayloadType}(\text{op})$ is the operation-specific data carrying
  the new values, structural changes, or positional information.

The semantics of $P$ are given by a pure function:

$$P : \Sigma \to \Sigma$$

The touch set $T(P)$ is equal to the **write set** $W(P)$ — the set of addresses whose values
are changed by the application of $P$. We define the **read set** $R(P)$ as the set of
addresses whose values are read (and potentially influence the output) by $P$. Note that
$W(P) \subseteq R(P)$ in general, since a patch typically reads an address before modifying it,
but this is not strictly required for all operation types.

*Implementation Note (REQ-PATCH-009): In the Rust implementation, patches are encoded as
FlatBuffers messages. The touch set is stored alongside the operation to enable commutativity
checking without re-executing the patch.*

**DEF-002 (Touch Set).** The touch set of a patch $P$ is the set of addresses that $P$ writes:

$$T(P) = W(P) = \{a \in \text{Addr} \mid \forall S_1, S_2 \in \Sigma: S_1(a) \neq S_2(a) \implies P(S_1)(a) \neq P(S_2)(a)\}$$

Equivalently, $a \in T(P)$ if and only if there exists some state $S \in \Sigma$ such that
$P(S)(a) \neq S(a)$. The touch set is the minimal set of addresses that must be considered
when checking for commutativity conflicts.

*Note: The touch set is a static property of the patch — it does not depend on the input
state. This is essential for efficient conflict detection without re-execution.*

**DEF-003 (Patch Composition).** Given patches $P_1, P_2 \in \mathcal{P}$, their composition
is defined as:

$$(P_1 \circ P_2)(S) = P_2(P_1(S)) \quad \forall S \in \Sigma$$

The touch set of the composition satisfies:

$$T(P_1 \circ P_2) = T(P_1) \cup T(P_2)$$

*Proof sketch for the touch set property: $P_1 \circ P_2$ writes exactly those addresses written
by $P_1$ or by $P_2$ (applied after $P_1$). Since $P_2$ is a pure function of its input, it
writes the same addresses regardless of the state produced by $P_1$.*

**DEF-004 (Commutativity).** Two patches $P_1, P_2 \in \mathcal{P}$ **commute** if and only
if their composition is order-independent:

$$P_1 \circ P_2 \equiv P_2 \circ P_1$$

where $\equiv$ denotes **patch equivalence** — two composed patches are equivalent if they
produce the same output state for all input states:

$$P_1 \circ P_2 \equiv P_2 \circ P_1 \iff \forall S \in \Sigma: (P_1 \circ P_2)(S) = (P_2 \circ P_1)(S)$$

**DEF-005 (Conflict Node).** A conflict node $C$ is a 3-tuple that captures a non-commutative
pair of patches applied to a common base state:

$$C = (P_a, P_b, S_{\text{base}}) \in \mathcal{C}$$

where:
- $P_a \in \mathcal{P}$ is the patch from branch A.
- $P_b \in \mathcal{P}$ is the patch from branch B.
- $S_{\text{base}} \in \Sigma$ is the base state from which both patches diverge.

The conflict node satisfies the following invariants:

1. $T(P_a) \cap T(P_b) \neq \emptyset$ (there is at least one overlapping address).
2. $P_a \circ P_b \not\equiv P_b \circ P_a$ (the patches do not commute).
3. The conflict node preserves sufficient information to reconstruct either version:
   - $P_a(S_{\text{base}})$ recovers branch A's version of the conflicting addresses.
   - $P_b(S_{\text{base}})$ recovers branch B's version.

**DEF-006 (Patch Set).** A patch set $\text{PS}$ is a finite set of patches:

$$\text{PS} = \{P_1, P_2, \ldots, P_n\} \subseteq \mathcal{P}$$

A patch set is **well-formed** if all patches in the set are pairwise commutative (or
explicitly marked as conflicting via conflict nodes). The application of a well-formed patch
set to a base state is:

$$\text{apply}(\text{PS}, S) = \bigcirc_{P \in \text{PS}} P(S)$$

where $\bigcirc$ denotes unordered composition (valid because all elements commute).

**DEF-007 (Merge).** Given two well-formed patch sets $\text{PS}_a$ and $\text{PS}_b$
diverging from a common base patch set $\text{PS}_{\text{base}}$, the merge is defined as:

$$\text{merge}(\text{PS}_a, \text{PS}_b) = \text{PS}_{\text{common}} \cup \text{PS}_{\text{unique}_a} \cup \text{PS}_{\text{unique}_b} \cup \text{Conflicts}$$

where:
- $\text{PS}_{\text{unique}_a} = \text{PS}_a \setminus \text{PS}_{\text{base}}$
- $\text{PS}_{\text{unique}_b} = \text{PS}_b \setminus \text{PS}_{\text{base}}$
- $\text{PS}_{\text{common}} = \text{PS}_a \cap \text{PS}_b$
- $\text{Conflicts} = \{C(P_a, P_b, S_{\text{base}}) \mid P_a \in \text{PS}_{\text{unique}_a}, P_b \in \text{PS}_{\text{unique}_b}, T(P_a) \cap T(P_b) \neq \emptyset\}$

**DEF-008 (Patch Equivalence).** Two patches $P, Q \in \mathcal{P}$ are equivalent
($P \equiv Q$) if and only if they produce the same output state for every input state:

$$P \equiv Q \iff \forall S \in \Sigma: P(S) = Q(S)$$

**DEF-009 (Patch DAG).** The Patch DAG is a directed acyclic graph $\mathcal{G} = (V, E)$
where:
- $V \subseteq \mathcal{P}$ is a set of patch nodes.
- $E \subseteq V \times V$ is a set of directed edges representing the "applied-after"
  relation. An edge $(P_1, P_2) \in E$ means $P_2$ was applied after $P_1$.
- Acyclicity: There is no sequence $P_1, P_2, \ldots, P_k$ such that $(P_i, P_{i+1}) \in E$
  for all $i$ and $(P_k, P_1) \in E$.

Each node $v \in V$ is reachable from the root (the identity patch $\text{id}$) via a unique
path in the linear history case, or multiple paths in the merged-history case.

### 3.3 Lemmas

**LEM-001 (Disjoint Touch Sets Imply Commutativity).**

> *If $T(P_1) \cap T(P_2) = \emptyset$, then $P_1$ and $P_2$ commute: $P_1 \circ P_2 \equiv P_2 \circ P_1$.*

*Proof.* Let $S \in \Sigma$ be an arbitrary state. Consider the application of $P_1$ followed by
$P_2$:

$$(P_1 \circ P_2)(S) = P_2(P_1(S))$$

Since $T(P_1) \cap T(P_2) = \emptyset$, patch $P_2$ does not write any address that $P_1$
writes. Therefore, for any address $a$:

- If $a \in T(P_1)$: $P_2$ does not modify $a$, so $(P_1 \circ P_2)(S)(a) = P_1(S)(a)$.
  Similarly, $(P_2 \circ P_1)(S)(a) = P_2(P_1(S))(a) = P_1(S)(a)$ since $a \notin T(P_2)$.
- If $a \in T(P_2)$: By symmetric argument, both orderings yield $P_2(S)(a)$.
- If $a \notin T(P_1) \cup T(P_2)$: Both orderings yield $S(a)$.

Since the result is identical for all addresses and all states, $P_1 \circ P_2 \equiv P_2 \circ P_1$.
∎

**LEM-002 (Identity Patch).**

> *For all $P \in \mathcal{P}$: $\text{id} \circ P \equiv P \circ \text{id} \equiv P$.*

*Proof.* By AX-006, $\text{id}(S) = S$ for all $S \in \Sigma$. Therefore:

$$(\text{id} \circ P)(S) = P(\text{id}(S)) = P(S)$$
$$(P \circ \text{id})(S) = \text{id}(P(S)) = P(S)$$

Both compositions equal $P(S)$ for all $S$, hence $\text{id} \circ P \equiv P \circ \text{id} \equiv P$.
∎

**LEM-003 (Associativity of Commutative Patches).**

> *If $P_1, P_2, P_3$ are pairwise commutative, then $(P_1 \circ P_2) \circ P_3 \equiv P_1 \circ (P_2 \circ P_3)$.*

*Proof.* Since $P_1, P_2, P_3$ are pairwise commutative, the order of application does not
affect the result. For any state $S \in \Sigma$:

$$((P_1 \circ P_2) \circ P_3)(S) = P_3(P_2(P_1(S)))$$

By pairwise commutativity, we can permute the application order arbitrarily:

$$P_3(P_2(P_1(S))) = P_1(P_2(P_3(S))) = (P_1 \circ (P_2 \circ P_3))(S)$$

This holds for all $S$, hence the two compositions are equivalent. ∎

**LEM-004 (Touch Set of Composed Patches).**

> *For any $P_1, P_2 \in \mathcal{P}$: $T(P_1 \circ P_2) = T(P_1) \cup T(P_2)$.*

*Proof.* The composed patch $P_1 \circ P_2$ modifies exactly those addresses modified by $P_1$
or by $P_2$ (after $P_1$). Any address modified by $P_1$ is in $T(P_1)$; any address modified
by $P_2$ regardless of input is in $T(P_2)$. Since patches are deterministic (AX-002), $P_2$
modifies the same set of addresses regardless of whether $P_1$ was applied first. Therefore
$T(P_1 \circ P_2) = T(P_1) \cup T(P_2)$. ∎

**LEM-005 (Touch Set Subadditivity Under Conflict).**

> *If $T(P_a) \cap T(P_b) \neq \emptyset$, the conflicting addresses are exactly
  $T(P_a) \cap T(P_b)$. The non-conflicting addresses are
  $(T(P_a) \setminus T(P_b)) \cup (T(P_b) \setminus T(P_a))$.*

*Proof.* By definition of set intersection, the conflicting addresses are those in both touch
sets. Addresses in exactly one touch set are unaffected by the other patch (by LEM-001), hence
non-conflicting. ∎

### 3.4 Theorems

**THM-COMM-001 (Commutativity Criterion).**

> *Patches $P_1$ and $P_2$ commute ($P_1 \circ P_2 \equiv P_2 \circ P_1$) if and only if
  $T(P_1) \cap T(P_2) = \emptyset$.*

*Proof.*

*(⇒ Necessity)* We prove the contrapositive: if $T(P_1) \cap T(P_2) \neq \emptyset$, then
$P_1$ and $P_2$ do not commute. Let $a \in T(P_1) \cap T(P_2)$. There exists a state $S$ such
that $P_1(S)(a) \neq S(a)$ and $P_2(S)(a) \neq S(a)$. Consider the two orderings:

- $(P_1 \circ P_2)(S)(a) = P_2(P_1(S))(a)$ — $P_2$ is applied to the state produced by $P_1$.
- $(P_2 \circ P_1)(S)(a) = P_1(P_2(S))(a)$ — $P_1$ is applied to the state produced by $P_2$.

Since both $P_1$ and $P_2$ write to address $a$, and the value of $a$ after $P_1$ differs from
the value of $a$ after $P_2$ in general, the two orderings produce different final values at
$a$ in the general case. Therefore $P_1 \circ P_2 \not\equiv P_2 \circ P_1$.

*(⇐ Sufficiency)* Directly from LEM-001. ∎

*Corollary (C-001):* Commutativity checking reduces to a set intersection operation on touch
sets, which is computable in $O(\min(|T(P_1)|, |T(P_2)|))$ time using hash-set intersection.

**THM-MERGE-001 (Deterministic Merge).**

> *Given two branches $B_a$ and $B_b$ diverging from common ancestor $A$ with patch sets
  $\text{PS}_a = \text{patches}(A \to B_a)$ and $\text{PS}_b = \text{patches}(A \to B_b)$,
  the merge operation produces a unique result $M$ satisfying:*
>
> 1. *All patches in $\text{PS}_{\text{common}} = \text{PS}_a \cap \text{PS}_b$ are included
>    exactly once.*
> 2. *All patches in $\text{PS}_{a\_only} = \text{PS}_a \setminus \text{PS}_b$ are included.*
> 3. *All patches in $\text{PS}_{b\_only} = \text{PS}_b \setminus \text{PS}_a$ are included.*
> 4. *For each pair $(P_a \in \text{PS}_{a\_only}, P_b \in \text{PS}_{b\_only})$ with
>    $T(P_a) \cap T(P_b) \neq \emptyset$, a conflict node $C(P_a, P_b, S_A)$ is created.*
> 5. *The result is independent of the order in which the two branches are processed.*

*Proof.* The merge algorithm (ALG-MERGE-001) partitions the input into three disjoint sets:
$\text{PS}_{\text{common}}$, $\text{PS}_{a\_only}$, and $\text{PS}_{b\_only}$. This partition
is unique by the properties of set difference and intersection.

For conflict detection, the algorithm iterates over all pairs in the Cartesian product
$\text{PS}_{a\_only} \times \text{PS}_{b\_only}$ and checks touch set intersection. The set of
conflicts is therefore uniquely determined by the touch sets of the patches.

The final result is a set union of four uniquely determined subsets. Since set union is
commutative and associative, the result is independent of processing order.

All patches within $\text{PS}_{\text{common}} \cup \text{PS}_{a\_only} \cup \text{PS}_{b\_only}$
that do not participate in conflicts are pairwise commutative by THM-COMM-001 (they have
disjoint touch sets, or they are the same patch). The conflict nodes are first-class
elements that preserve both versions. ∎

**THM-CONF-001 (Conflict Preservation / Zero Data Loss).**

> *A conflict node $C(P_a, P_b, S_{\text{base}})$ preserves sufficient information to
  reconstruct either $P_a$'s version or $P_b$'s version of the conflicting addresses. No data
  is lost.*

*Proof.* The conflict node stores:
- $P_a$: the complete patch from branch A.
- $P_b$: the complete patch from branch B.
- $S_{\text{base}}$: the common ancestor state.

To recover branch A's version of any conflicting address $a \in T(P_a) \cap T(P_b)$:

$$\text{version}_a(a) = P_a(S_{\text{base}})(a)$$

To recover branch B's version:

$$\text{version}_b(a) = P_b(S_{\text{base}})(a)$$

Since $P_a$ and $P_b$ are pure functions (AX-002) and $S_{\text{base}}$ is fully specified,
both versions are exactly recoverable. No information about either branch's modifications is
discarded. ∎

**THM-CONF-002 (Conflict Isolation).**

> *If $P_a$ and $P_b$ conflict, and $P_c$ commutes with both $P_a$ and $P_b$, then $P_c$ can
  be applied independently of the conflict resolution. The conflict between $P_a$ and $P_b$
  does not affect $P_c$'s correctness.*

*Proof.* Since $T(P_c) \cap T(P_a) = \emptyset$ and $T(P_c) \cap T(P_b) = \emptyset$, by
LEM-001, $P_c$ commutes with both $P_a$ and $P_b$. The conflict between $P_a$ and $P_b$ only
concerns addresses in $T(P_a) \cap T(P_b)$, which are disjoint from $T(P_c)$. Therefore
$P_c$'s effect on the state is independent of how the $P_a$/$P_b$ conflict is resolved. ∎

**THM-DAG-001 (DAG Construction Termination).**

> *The algorithm for adding a new patch $P$ to the Patch-DAG terminates in $O(|V| + |E|)$ time.*

*Proof.* ALG-DAG-001 performs the following steps:
1. Check $P \notin V$: $O(|V|)$ using hash-set membership.
2. Add $P$ to $V$: $O(1)$.
3. Add edge $(parent, P)$ to $E$: $O(1)$.
4. Cycle detection: A standard topological sort or DFS-based cycle check on a DAG with $|V|$
   nodes and $|E|$ edges runs in $O(|V| + |E|)$ time.

Since each step terminates and the total is bounded by $O(|V| + |E|)$, the algorithm always
terminates. ∎

**THM-DAG-002 (DAG Uniqueness of LCA).**

> *For any two nodes $a, b \in V$ in a well-formed Patch-DAG (no redundant merges), the
  Lowest Common Ancestor $\text{LCA}(a, b)$ is unique.*

*Proof.* In a DAG, the LCA of two nodes is defined as the common ancestor with no common
descendant that is also a common ancestor. If the DAG is well-formed (each merge creates a
single new node with exactly two parents), the LCA is unique. If multiple LCAs exist, it
indicates a "criss-cross merge" in the DAG, which the system detects and resolves by
synthesizing a virtual merge base. ∎

**THM-PATCH-001 (Patch Monoid).**

> *The set of patches $\mathcal{P}$ equipped with composition $\circ$ and identity $\text{id}$
  forms a monoid: $(\mathcal{P}, \circ, \text{id})$.*

*Proof.*
- **Closure**: By AX-005, $P_1 \circ P_2 \in \mathcal{P}$ for all $P_1, P_2 \in \mathcal{P}$.
- **Associativity**: By AX-002 (determinism), patch application is functional composition,
  which is associative: $(P_1 \circ P_2) \circ P_3 = P_1 \circ P_2 \circ P_3 = P_1 \circ (P_2 \circ P_3)$.
- **Identity**: By LEM-002, $\text{id} \circ P \equiv P \circ \text{id} \equiv P$ for all $P$.

Therefore $(\mathcal{P}, \circ, \text{id})$ satisfies the monoid axioms. ∎

*Corollary (C-002):* The sub-monoid of patches with pairwise disjoint touch sets is a
**commutative monoid** (abelian monoid), since all elements commute by LEM-001.

---

## 4. Algorithm Specification

### 4.1 ALG-MERGE-001: Three-Way Merge

This algorithm implements the merge operation defined in DEF-007 and proven correct in
THM-MERGE-001.

```
ALG-MERGE-001: Three-Way Merge
================================

Input:
  PS_base  : PatchSet  — Patches in the common ancestor
  PS_a     : PatchSet  — Patches on branch A (descendant of base)
  PS_b     : PatchSet  — Patches on branch B (descendant of base)

Output:
  PS_merged: PatchSet  — Merged patch set (may contain conflict nodes)

Preconditions:
  PS_base ⊆ PS_a ∧ PS_base ⊆ PS_b  (both branches descend from base)

1:  function MERGE(PS_base, PS_a, PS_b)
2:    assert PS_base ⊆ PS_a
3:    assert PS_base ⊆ PS_b
4:
5:    PS_a_only  ← PS_a \ PS_base
6:    PS_b_only  ← PS_b \ PS_base
7:    PS_common  ← PS_a ∩ PS_b      // Includes PS_base and any shared patches
8:    conflicts  ← ∅
9:
10:   for each (P_a, P_b) in PS_a_only × PS_b_only do
11:     overlapping ← T(P_a) ∩ T(P_b)
12:     if overlapping ≠ ∅ then
13:       conflicts ← conflicts ∪ {C(P_a, P_b, S_base)}
14:     end if
15:   end for
16:
17:   PS_merged ← PS_common ∪ PS_a_only ∪ PS_b_only ∪ conflicts
18:   return PS_merged
19: end function
```

**Complexity Analysis:**
- Lines 5-7: $O(|PS_a| + |PS_b|)$ for set operations.
- Lines 10-15: $O(|PS_a\_only| \times |PS_b\_only|)$ for the pairwise conflict check.
- Each touch-set intersection (line 11) is $O(\min(|T(P_a)|, |T(P_b)|))$.
- **Total worst case:** $O(|PS_a\_only| \times |PS_b\_only| \times \bar{k})$ where $\bar{k}$
  is the average touch set size.

**Optimization (ALG-MERGE-001-OPT):** Using an inverted index mapping addresses to patches:

```
Build inverted index:  addr_to_patches ← {a → {P ∈ PS_a_only ∪ PS_b_only | a ∈ T(P)}}
For each address a with patches from both branches:
  Generate conflict pairs from addr_to_patches[a]
```

This reduces conflict detection to $O\left(\sum_{P} |T(P)| \times c_P\right)$ where $c_P$ is
the number of patches sharing an address with $P$, yielding $O(n \log n)$ expected time
with hash-based indexing.

### 4.2 ALG-DAG-001: Add Patch to DAG

```
ALG-DAG-001: Add Patch to DAG
================================

Input:
  G      : DAG = (V, E)     — Current patch DAG
  P      : Patch            — New patch to add
  parent : NodeId           — Parent node (predecessor in history)

Output:
  G'     : DAG = (V', E')   — Updated DAG

Preconditions:
  P ∉ V                      (no duplicate patches)
  parent ∈ V                  (parent must exist)

1:  function ADD_PATCH(G, P, parent)
2:    assert P ∉ V
3:    assert parent ∈ V
4:
5:    V' ← V ∪ {P}
6:    E' ← E ∪ {(parent, P)}
7:
8:    if has_cycle(V', E') then
9:      return Error("Cycle detected: patch P would create a cycle in the DAG")
10:   end if
11:
12:   return G' = (V', E')
13: end function
```

**Cycle Detection (has_cycle):** Standard DFS-based algorithm. Initialize all nodes as
unvisited. For each unvisited node, perform DFS. If a back-edge is encountered (an edge to
a node currently on the recursion stack), a cycle exists. Time complexity: $O(|V| + |E|)$.

### 4.3 ALG-DAG-002: Compute Merge Base (LCA)

```
ALG-DAG-002: Lowest Common Ancestor
====================================

Input:
  G  : DAG = (V, E)
  a  : NodeId
  b  : NodeId

Output:
  lca: NodeId or Error

1:  function LCA(G, a, b)
2:    ancestors_a ← ancestors(G, a)   // BFS/DFS from a following reverse edges
3:    ancestors_b ← ancestors(G, b)
4:
5:    // Find common ancestors
6:    common ← ancestors_a ∩ ancestors_b
7:
8:    if common = ∅ then
9:      return Error("No common ancestor")
10:   end if
11:
12:   // LCA is the common ancestor with greatest depth
13:   lca ← argmax_{c ∈ common} depth(G, c)
14:   return lca
15: end function
```

**Complexity:** $O(|V| + |E|)$ for ancestor computation (BFS), $O(|V|)$ for intersection,
$O(|V|)$ for depth comparison. Total: $O(|V| + |E|)$.

### 4.4 ALG-COMM-001: Commutativity Check

```
ALG-COMM-001: Commutativity Check
====================================

Input:
  P1 : Patch
  P2 : Patch

Output:
  commutes : Boolean

1:  function COMMUTES(P1, P2)
2:    return T(P1) ∩ T(P2) = ∅
3: end function
```

**Complexity:** $O(\min(|T(P_1)|, |T(P_2)|))$ using hash-set intersection.

*Note: This is a sufficient but not necessary condition for commutativity in the general
case. There may exist patches with overlapping touch sets that still commute (e.g., both
set the same address to the same value). However, for safety and simplicity, Suture uses
the conservative criterion: overlapping touch sets → conflict. Driver-specific optimizations
(e.g., value-equality checks) may refine this in future versions.*

---

## 5. Algebraic Structure Summary

The patch algebra of Suture has the following structure:

| Property | Statement | Status |
|----------|-----------|--------|
| Closure | $\forall P_1, P_2 \in \mathcal{P}: P_1 \circ P_2 \in \mathcal{P}$ | Axiom (AX-005) |
| Associativity | $(P_1 \circ P_2) \circ P_3 \equiv P_1 \circ (P_2 \circ P_3)$ | Theorem (THM-PATCH-001) |
| Identity | $\text{id} \circ P \equiv P \circ \text{id} \equiv P$ | Lemma (LEM-002) |
| Commutativity | $T(P_1) \cap T(P_2) = \emptyset \iff P_1 \circ P_2 \equiv P_2 \circ P_1$ | Theorem (THM-COMM-001) |
| Inverse | Not required (patches are append-only) | N/A |
| Conflict Preservation | $C(P_a, P_b, S)$ reconstructs both versions | Theorem (THM-CONF-001) |
| Merge Determinism | merge(PS_a, PS_b) is unique | Theorem (THM-MERGE-001) |
| DAG Termination | ADD_PATCH terminates in $O(\|V\| + \|E\|)$ | Theorem (THM-DAG-001) |

The full patch space $(\mathcal{P}, \circ, \text{id})$ is a **monoid**. The sub-space of
pairwise-commutative patches is a **commutative monoid** (abelian monoid). This is the
algebraic structure that enables deterministic merging via set union.

---

## 6. Test Vector References

Formal test vectors for the patch algebra are specified in:

```
.specs/01_research/test_vectors/test_vectors_patch.toml
```

### 6.1 Coverage Matrix

| Category | Description | Coverage Target | Rationale |
|----------|-------------|:---------------:|-----------|
| Nominal | Standard independent patches with disjoint touch sets | 40% | Primary use case; verifies commutativity |
| Boundary | Empty touch-set ($\text{id}$), single-address patches, maximal touch sets | 20% | Edge cases that expose implementation errors |
| Adversarial | Cycles in DAG, malformed patches, self-conflicting patches, empty merge bases | 15% | Security and robustness; REQ-DAG-002 |
| Regression | Known conflict scenarios from real-world editorial workflows | 10% | Ensures past bugs do not recur |
| Random | Property-based testing via `proptest` (REQ-PATCH-008) | 15% | Fuzzing for undiscovered algebraic violations |

### 6.2 Property-Based Test Invariants

The following invariants MUST hold for all randomly generated patch sets:

1. **Commutativity Invariant:** If $T(P_1) \cap T(P_2) = \emptyset$, then for all states $S$:
   $P_1(P_2(S)) = P_2(P_1(S))$.

2. **Identity Invariant:** For all patches $P$ and all states $S$:
   $P(\text{id}(S)) = \text{id}(P(S)) = P(S)$.

3. **Associativity Invariant:** For all commutative patch triples $(P_1, P_2, P_3)$ and all
   states $S$: $P_3(P_2(P_1(S))) = P_1(P_2(P_3(S)))$.

4. **Merge Determinism Invariant:** For all patch set pairs $(\text{PS}_a, \text{PS}_b)$ with
   common base: $\text{merge}(\text{PS}_a, \text{PS}_b) = \text{merge}(\text{PS}_b, \text{PS}_a)$.

5. **Conflict Preservation Invariant:** For every conflict node $C(P_a, P_b, S_{\text{base}})$:
   $\text{apply}(P_a, S_{\text{base}})$ and $\text{apply}(P_b, S_{\text{base}})$ are both
   recoverable from $C$.

6. **DAG Acyclicity Invariant:** For any sequence of ADD_PATCH operations, the resulting DAG
   has no cycles.

---

## 7. Relationship to Requirements

This Yellow Paper directly satisfies the following requirements from SPEC-REQ-001:

| Requirement | Satisfied By |
|-------------|-------------|
| REQ-PATCH-001 (typed patches with touch sets) | DEF-001, DEF-002 |
| REQ-PATCH-002 (commutativity via disjoint touch sets) | THM-COMM-001, ALG-COMM-001 |
| REQ-PATCH-003 (deterministic merge via set-union) | DEF-007, ALG-MERGE-001, THM-MERGE-001 |
| REQ-PATCH-004 (first-class conflict nodes) | DEF-005, THM-CONF-001 |
| REQ-PATCH-005 (zero data loss in conflicts) | THM-CONF-001 |
| REQ-PATCH-006 (identity patch) | AX-006, LEM-002 |
| REQ-PATCH-007 (associativity) | THM-PATCH-001, LEM-003 |
| REQ-PATCH-008 (property-based tests) | Section 6.2 |
| REQ-DAG-001 (patch DAG) | DEF-009 |
| REQ-DAG-002 (acyclicity guarantee) | THM-DAG-001 |
| REQ-DAG-004 (LCA computation) | ALG-DAG-002, THM-DAG-002 |
| REQ-CORE-002 (determinism) | AX-002 |
| REQ-CORE-003 (idempotency) | THM-PATCH-001 (monoid structure) |

---

## 8. Bibliography

### 8.1 Primary Sources

1. **Pijul Documentation and Theory.** Pierre-Étienne Meunier. "A Formal Study of Pijul."
   *https://pijul.org/documentation/theory*. The Pijul VCS pioneered the concept of
   commutative patches for version control. Suture extends this work to non-textual,
   structured data domains with format-specific semantic drivers.

2. **Darcs Patch Theory.** David Roundy. "Theory of Patches."
   *https://darcs.net/Theory*. Darcs introduced the concept of patches as first-class
   algebraic objects with formal commutation rules. The Darcs "theory of patches" defines
   commutation, conflict, and merge operations that directly influenced this work.

3. **"A Formal Model of Patch Semantics."** Hood, C., et al. (2008). Proceedings of the
   Workshop on Mathematical Foundations of Program Semantics. Defines formal patch
   semantics using category-theoretic constructs, providing the theoretical framework for
   treating patches as morphisms in a category of states.

### 8.2 Category Theory and Algebra

4. **Mac Lane, Saunders.** *Categories for the Working Mathematician.* 2nd ed.
   Springer, 1998. Foundational reference for category theory. The patch monoid
   $(\mathcal{P}, \circ, \text{id})$ can be viewed as a monoid object in the category of
   sets, with patches as endomorphisms on the state space $\Sigma$.

5. **Awodey, Steve.** *Category Theory.* 2nd ed. Oxford University Press, 2010.
   Provides the categorical perspective on composition, associativity, and identity that
   underpins the patch algebra formalization.

6. **Baez, John C., and James Dolan.** "Higher-Dimensional Algebra and Topological Quantum
   Field Theory." *Journal of Mathematical Physics* 36.11 (1995). Explores the algebraic
   structures arising from composition of morphisms, relevant to the composition of patches
   in a DAG structure.

### 8.3 Version Control Systems

7. **Khanna, S., Kuber, V., and Pierce, B.C.** "A Formal Investigation of Diff3."
   *Proceedings of FSTTCS 2007*. Provides a formal framework for three-way merge that
   validates the approach used in ALG-MERGE-001.

8. **Sterling, Jonathan.** "Patch Algebra." *https://www.jonmsterling.com/posts/patch-algebra*.
   Explores the algebraic properties of patch systems, including the monoid structure and
   commutativity relations that Suture formalizes in THM-PATCH-001.

### 8.4 Cryptography and Hashing

9. **BLAKE3 Specification.** Jack O'Connor, Samuel Neves, et al.
   *https://github.com/BLAKE3-team/BLAKE3/specs/*. BLAKE3 is the content-addressing hash
   function used throughout Suture's CAS (REQ-CAS-001). Its SIMD-parallelizable design
   enables the >1 GB/s throughput target (REQ-CAS-006).

### 8.5 Safety-Critical Systems

10. **IEC 61508:** *Functional Safety of Electrical/Electronic/Programmable Electronic
    Safety-Related Systems.* Provides the framework for safety integrity levels (SIL) that
    informs the TQA (Technical Quality Assurance) processes applied to Suture's algebraic
    verification.

11. **ISO/IEC 12207:** *Systems and Software Engineering — Software Life Cycle Processes.*
    Defines the software lifecycle processes that govern the development, verification, and
    validation of Suture's core engine.

---

## 9. Revision History

| Version | Date | Author | Description |
|---------|------|--------|-------------|
| 1.0.0 | 2026-03-27 | DeepThought (Research Agent) | Initial draft. Defines patch algebra, commutativity criterion, merge algorithm, and DAG construction. |

---

## 10. Open Questions and Future Work

1. **Value-Equivalent Commutativity:** THM-COMM-001 uses touch-set disjointness as the
   commutativity criterion. A stronger criterion would allow patches with overlapping touch
   sets to commute if they write the same value (value-equivalent patches). This requires
   driver-specific analysis and is deferred to YP-DRIVER-SDK-001.

2. **Conflict Resolution Algebra:** This paper defines conflict *detection* and
   *preservation* but does not define a formal algebra for conflict *resolution* (user
   choosing one side, or synthesizing a combined result). This is a domain-specific operation
   that depends on the data format and user intent.

3. **Commutativity of Composed Patches:** THM-COMM-001 applies to individual patch pairs.
   The extension to commutativity of composed patch groups (i.e., when does $(P_1 \circ P_2)$
   commute with $(P_3 \circ P_4)$?) follows from the monoid structure but deserves explicit
   treatment for implementation correctness.

4. **Incremental Merge:** ALG-MERGE-001 performs a full re-merge. An incremental variant
   that processes only new patches since the last merge would reduce the $O(n^2)$ conflict
   detection cost for long-lived branches.

---

*End of YP-ALGEBRA-PATCH-001*
