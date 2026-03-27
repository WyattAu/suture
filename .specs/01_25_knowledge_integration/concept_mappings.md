---
document_id: SPEC-CM-001
version: 1.0.0
status: DRAFT
phase: 1.25
created: 2026-03-27
author: Cross-Lingual Integration Agent
confidence_level: 0.92
---

# Concept Mappings: Cross-Lingual Terminology

## 1. Core Patch Theory Concepts

| EN Term | ZH Term | JP Term | Definition | Source | Confidence |
|---------|---------|---------|------------|--------|------------|
| Patch | 补丁 (bǔdīng) | パッチ (patti) | A semantic operation transforming project state; element of the patch monoid (P, o, id) | YP-ALGEBRA-PATCH-001, DEF-001 | 0.99 |
| Touch Set | 触及集 (chùjí jí) | タッチセット (tacchi setto) | Set of addresses modified by a patch; T(P) = W(P), the write set | YP-ALGEBRA-PATCH-001, DEF-002 | 0.95 |
| Commutativity | 交换性 (jiāohuàn xìng) | 交換法則 (kōkan hōsoku) | Order-independence of operations: P1 o P2 = P2 o P1 | Mathematics; YP-ALGEBRA-PATCH-001, DEF-004 | 0.99 |
| Merge | 合并 (hébìng) | マージ (māji) | Combining divergent patch histories into a unified set | VCS Theory; YP-ALGEBRA-PATCH-001, DEF-007 | 0.99 |
| Conflict | 冲突 (chōngtū) | 競合 (kyōgō) | Non-commutative patch pair requiring explicit resolution | VCS Theory; YP-ALGEBRA-PATCH-001, DEF-005 | 0.99 |
| Conflict Node | 冲突节点 (chōngtū jiédiǎn) | 競合ノード (kyōgō nōdo) | First-class DAG element preserving both sides of a conflict | YP-ALGEBRA-PATCH-001, DEF-005 | 0.95 |
| Patch Composition | 补丁组合 (bǔdīng zǔhé) | パッチ合成 (patti gōsei) | Sequential application: (P1 o P2)(S) = P2(P1(S)) | YP-ALGEBRA-PATCH-001, DEF-003 | 0.99 |
| Identity Patch | 单位补丁 (dānwèi bǔdīng) | 恒等パッチ (kōtō patti) | Neutral element: id(S) = S for all S | YP-ALGEBRA-PATCH-001, AX-006, LEM-002 | 0.99 |
| Patch Equivalence | 补丁等价 (bǔdīng děngjià) | パッチ同値 (patti dōchi) | P = Q iff P(S) = Q(S) for all S | YP-ALGEBRA-PATCH-001, DEF-008 | 0.99 |

## 2. Algebraic Structure Concepts

| EN Term | ZH Term | JP Term | Definition | Source | Confidence |
|---------|---------|---------|------------|--------|------------|
| Monoid | 幺半群 (yāobànqún) | モノイド (monoido) | Set with associative binary operation and identity element | Abstract Algebra; YP-ALGEBRA-PATCH-001, THM-PATCH-001 | 0.99 |
| Commutative Monoid | 交换幺半群 (jiāohuàn yāobànqún) | 可換モノイド (kakan monoido) | Monoid where all elements commute: a o b = b o a | Abstract Algebra; YP-ALGEBRA-PATCH-001, C-002 | 0.99 |
| Associativity | 结合性 (jiéhé xìng) | 結合法則 (ketsugō hōsoku) | (a o b) o c = a o (b o c) | Mathematics; YP-ALGEBRA-PATCH-001, LEM-003 | 0.99 |
| Determinism | 确定性 (quèdìng xìng) | 決定性 (ketteisei) | Same input always produces same output | REQ-CORE-002; YP-ALGEBRA-PATCH-001, AX-002 | 0.99 |
| Idempotency | 幂等性 (mìděng xìng) | 冪等性 (mītōsei) | f(f(x)) = f(x) | REQ-CORE-003; derived from monoid structure | 0.99 |

## 3. Graph and Data Structure Concepts

| EN Term | ZH Term | JP Term | Definition | Source | Confidence |
|---------|---------|---------|------------|--------|------------|
| Directed Acyclic Graph (DAG) | 有向无环图 (yǒuxiàng wúhuán tú) | 有向非巡回グラフ (yūkōhi junkaizu) | Graph with no directed cycles | Graph Theory; YP-ALGEBRA-PATCH-001, DEF-009 | 0.99 |
| Lowest Common Ancestor (LCA) | 最近公共祖先 (zuìjìn gōnggòng zǔxiān) | 最近共通祖先 (saikin kyōtsū sosen) | Deepest node that is an ancestor of both query nodes | Graph Theory; YP-ALGEBRA-PATCH-001, ALG-DAG-002 | 0.99 |
| Content Addressable Storage (CAS) | 内容可寻址存储 (nèiróng kě xúnzhǐ cúnchǔ) | コンテンツアドレッサブルストレージ | Storage indexed by content hash (BLAKE3) | Systems; domain_analysis.md, Section 2.2 | 0.95 |
| Patch Set | 补丁集 (bǔdīng jí) | パッチ集合 (patti shūgō) | Finite set of patches; well-formed if pairwise commutative | YP-ALGEBRA-PATCH-001, DEF-006 | 0.99 |
| Patch DAG | 补丁有向无环图 (bǔdīng yǒuxiàng wúhuán tú) | パッチDAG (patti DAG) | DAG of patches representing full project history | YP-ALGEBRA-PATCH-001, DEF-009 | 0.95 |

## 4. Collaborative Editing and OT Concepts

| EN Term | ZH Term | JP Term | Definition | Source | Confidence |
|---------|---------|---------|------------|--------|------------|
| Operational Transformation (OT) | 操作变换 (cāozuò biànhuàn) | 操作変換 (sōsa henkan) | Real-time collaborative editing technique using transformation functions | CSCW; Ellis & Gibbs 1989 | 0.90 |
| Transformation Function | 变换函数 (biànhuàn hánshù) | 変換関数 (henkan kansū) | Function T(O1, O2) adjusting O2 for the effects of O1 | OT Theory | 0.90 |
| Convergence | 收敛 (shōuliǎn) | 収束 (shūsoku) | All replicas reach the same final state | Distributed Systems | 0.95 |
| Causality | 因果关系 (yīnguǒ guānxì) | 因果関係 (inga kankei) | Ordering constraint ensuring causal dependencies are respected | Distributed Systems | 0.99 |
| TP1 Property | TP1 属性 (TP1 shǔxìng) | TP1 性質 (TP1 seishitsu) | T(O1, O2) preserves O2's intended effect | OT Theory | 0.90 |
| TP2 Property | TP2 属性 (TP2 shǔxìng) | TP2 性質 (TP2 seishitsu) | O1 and T(O1, O2) commute | OT Theory | 0.90 |

## 5. Suture-Specific Concepts

| EN Term | ZH Term | JP Term | Definition | Source | Confidence |
|---------|---------|---------|------------|--------|------------|
| Semantic Versioning | 语义版本控制 (yǔyì bǎnběn kòngzhì) | セマンティックバージョニング | Meaning-aware version control using patch algebra | Suture-specific; domain_analysis.md | 0.85 |
| Driver | 驱动程序 (qūdòng chéngxù) | ドライバー (doraibā) | Plugin implementing SutureDriver trait for a specific file format | Suture-specific; domain_analysis.md | 0.95 |
| Virtual File System (VFS) | 虚拟文件系统 (xūnǐ wénjiàn xìtǒng) | 仮想ファイルシステム (kasō fairu shisutemu) | User-space file virtualization layer | Suture-specific; domain_analysis.md | 0.95 |
| Hub | 中心服务 (zhōngxīn fúwù) | ハブ (habu) | Enterprise coordination service (Raft + PostgreSQL + S3) | Suture-specific; domain_analysis.md | 0.95 |
| Intermediate Representation (IR) | 中间表示 (zhōngjiān biǎoshì) | 中間表現 (chūkan hyōgen) | Driver-specific serialized form of a patch | Suture-specific; domain_analysis.md | 0.95 |
| Lease | 租约 (zūyuē) | リース (rīsu) | Time-bound exclusive lock on a non-mergeable resource | Suture-specific; domain_analysis.md | 0.95 |
| Merge Paralysis | 合并瘫痪 (hébìng tānhuàn) | マージ麻痺 (māji mahi) | Full-file conflicts on semantically independent changes | Suture-specific; domain_analysis.md | 0.90 |
| Read Set | 读取集 (dǔqǔ jí) | リードセット (rīdo setto) | Set of addresses read by a patch: R(P) | YP-ALGEBRA-PATCH-001, Section 3.2 | 0.95 |
| Write Set | 写入集 (xiěrù jí) | ライトセット (raito setto) | Set of addresses written by a patch: W(P) = T(P) | YP-ALGEBRA-PATCH-001, DEF-002 | 0.95 |

## 6. Notes on Translation Quality

- **Chinese (ZH)** translations follow standard mathematical and CS terminology as used in
  mainland Chinese textbooks and CNKI-indexed papers. Traditional Chinese (ZH-TW) terms
  may differ (e.g., 幺半群 vs. 單元半群).
- **Japanese (JP)** translations follow standard CS terminology used in JSAI and IPSJ
  publications. Katakana renderings for loanwords (パッチ, マージ) are standard.
- **Korean (KO)** translations are deferred to Phase 2+ (P2 priority per domain_analysis.md).
- Terms marked with confidence < 0.90 are Suture-specific neologisms that lack established
  translations; these should be validated with native-speaking domain experts.
