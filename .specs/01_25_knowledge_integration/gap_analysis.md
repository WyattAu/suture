---
document_id: SPEC-GA-001
version: 1.0.0
status: DRAFT
phase: 1.25
created: 2026-03-27
author: Cross-Lingual Integration Agent
confidence_level: 0.85
---

# Gap Analysis: Missing Foundations and Research Needs

## 1. Missing Theoretical Foundations

### 1.1 Touch-Set Sufficiency for Real-World File Formats

**Gap:** THM-COMM-001 proves that touch-set disjointness is sufficient for commutativity,
but this theorem assumes a specific model of the state space (AX-001: state as a function
from addresses to values). For some file formats, this model may not hold:

- **XML/SVG:** Moving a node changes the XPath addresses of all subsequent siblings. A patch
  that moves node N and a patch that edits node M (where M follows N) have technically
  disjoint touch sets under a naive addressing scheme, but the move changes M's address,
  creating an implicit dependency.
- **OTIO Timelines:** Moving a clip changes the global timeline duration and the start times
  of all subsequent clips. A patch that moves clip A and a patch that adjusts the color of
  clip B (which appears after A) have disjoint touch sets but are semantically dependent.
- **Spreadsheet Formulas:** Editing cell A1 (which contains `=SUM(B1:B10)`) and inserting a
  row in the B1:B10 range have disjoint touch sets under cell-address indexing, but the
  formula reference changes.

**Impact:** If touch-set sufficiency fails for common formats, Suture will produce incorrect
merge results --- either missing real conflicts (silent data corruption) or flagging false
conflicts (merge paralysis).

**Severity:** High. Silent data corruption is the worst possible failure mode for a VCS.

### 1.2 Implicit Dependencies and Transitive Effects

**Gap:** The current formal model treats each patch's touch set as a static, self-contained
property. In practice, patches can have **implicit dependencies** --- side effects that
affect addresses not in the explicit touch set:

- A "move clip" operation affects the temporal positions of all subsequent clips.
- An "insert row" operation in a spreadsheet shifts all row references below it.
- A "resize container" operation in a UI layout may reflow child elements.

**Impact:** Patches that appear commutative (disjoint touch sets) may actually conflict
when implicit dependencies are considered.

**Severity:** Medium-High. This is a known limitation acknowledged in YP-ALGEBRA-PATCH-001
(Section 10, Open Question 1) but not formally addressed.

## 2. Research Needed

### 2.1 Empirical Study of OTIO Timeline Modification Patterns

**Objective:** Determine the optimal touch-set granularity for OTIO timelines by analyzing
real-world editorial workflows.

**Methodology:**
1. Collect a corpus of real OTIO timelines from partner studios (anonymized).
2. Instrument an OTIO editor to log all user modifications as patch operations.
3. Compute touch sets at varying granularities (clip-level, track-level, timeline-level).
4. Measure the commutativity rate (percentage of patch pairs with disjoint touch sets)
  at each granularity.
5. Identify common modification patterns that cause implicit dependencies.

**Success Criteria:**
- Identify a touch-set granularity that achieves > 95% commutativity rate for typical
  editorial workflows (defined as sessions with 2-5 concurrent editors).
- Document the remaining 5% of non-commutative pairs and classify their root causes.

**Effort Estimate:** 2-4 weeks of data collection + 1-2 weeks of analysis.

### 2.2 Benchmark Commutativity Detection for Editorial Workflows

**Objective:** Measure the false-positive rate of touch-set-based conflict detection against
a ground-truth oracle (manual expert classification of conflict vs. non-conflict).

**Methodology:**
1. Select 100 representative merge scenarios from real editorial sessions.
2. For each scenario, classify patch pairs as "true conflict" or "true commute" via manual
  expert review.
3. Run Suture's ALG-COMM-001 on the same pairs and compare.
4. Compute precision (of detected conflicts) and recall (of true conflicts).

**Success Criteria:**
- False positive rate < 10% (no more than 1 in 10 flagged conflicts is actually commutative).
- False negative rate = 0% (never miss a true conflict).

**Effort Estimate:** 1-2 weeks.

### 2.3 Formal Treatment of Implicit Dependencies

**Objective:** Extend the patch algebra to account for implicit dependencies without
sacrificing the O(min(|T(P1)|, |T(P2)|)) commutativity check cost.

**Approach Options:**
1. **Extended Touch Sets:** Include implicitly affected addresses in T(P). This is simple
   but may cause excessive false conflicts (a "move clip" patch would touch all subsequent
   clips, conflicting with almost any concurrent edit).
2. **Dependency Annotations:** Patches declare read dependencies on "positional contexts"
   (e.g., "depends on the start time of all preceding clips"). Two patches conflict if
   one's writes intersect the other's declared dependencies.
3. **Two-Phase Commutativity:** First check touch-set disjointness (fast path). If disjoint,
   check for implicit dependency conflicts (slow path, driver-specific).

**Recommendation:** Option 3 (two-phase) provides the best trade-off between correctness
and performance. The fast path handles 95%+ of cases; the slow path only runs for patches
that involve structural operations (move, insert, delete, resize).

## 3. Risk Assessment

### 3.1 Touch-Set Granularity Too Coarse

**Risk:** If touch sets are defined at too coarse a granularity (e.g., file-level instead
of clip-level), the commutativity rate drops to near-zero, and every merge produces
conflicts. This is the "merge paralysis" problem that Suture exists to solve.

**Likelihood:** Low. Driver implementations control touch-set granularity; the OTIO driver
will use clip-level or track-level addressing by default.

**Mitigation:** Driver SDK documentation must specify minimum touch-set granularity
requirements. Automated tests must verify that the driver's touch sets are fine-grained
enough for the target format.

### 3.2 Touch-Set Granularity Too Fine

**Risk:** If touch sets are defined at too fine a granularity (e.g., individual bytes within
a clip's metadata), unrelated patches may appear to conflict because they happen to modify
adjacent bytes. This produces excessive false conflicts, degrading user experience.

**Likelihood:** Medium. This is a real risk for formats where semantic units do not map
cleanly to addressable regions.

**Mitigation:** Driver implementations should group semantically related addresses into
single touch-set entries. The concept of "address" (Addr) in YP-ALGEBRA-PATCH-001 is
intentionally abstract --- a single address can represent a compound semantic unit.

### 3.3 XML/XPath Address Instability

**Risk:** For XML-based formats, the addressing scheme (XPath) is position-dependent.
Moving or inserting an element changes the XPath of all subsequent siblings, causing
cascading address changes.

**Likelihood:** Medium. Affects SVG, USD (partially), and any XML-serialized format.

**Mitigation:** Use stable, content-based addresses (e.g., UUIDs assigned to elements)
rather than position-based paths. This is the approach used by USD (asset paths) and
is recommended for all XML-based drivers.

### 3.4 OT Convergence Anomalies

**Risk:** If Suture is extended to support real-time collaborative editing (OT-style),
the weaker TP1/TP2 properties may introduce convergence anomalies --- scenarios where
two replicas reach different final states despite applying the same set of operations.

**Likelihood:** Low for v1.0 (Suture is batch-oriented). Medium for future real-time
extensions.

**Mitigation:** For v1.0, this is not a concern. For future extensions, adopt a proven OT
algorithm (e.g., Jupiter/Google Wave OT) and formally verify its integration with the
patch monoid structure.

## 4. Recommended Mitigations

| Risk | Mitigation | Priority | Owner | Target Date |
|------|-----------|----------|-------|-------------|
| Touch-set sufficiency gaps | Empirical study (2.1) | P0 | Research | Phase 2 |
| Implicit dependencies | Two-phase commutativity (2.3) | P1 | Core | Phase 2 |
| XML address instability | UUID-based addressing in drivers | P1 | Driver SDK | v1.0 |
| False positive conflicts | Benchmark study (2.2) | P1 | QA | Phase 2 |
| OT convergence anomalies | Defer to future phase; document constraint | P2 | Architecture | Post-v1.0 |
| Touch-set granularity calibration | Driver SDK guidelines + automated tests | P0 | Driver SDK | v1.0 |
