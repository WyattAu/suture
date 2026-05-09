//! Patch composition — collapsing a patch sequence into a single equivalent patch.
//!
//! # Theory
//!
//! DEF-COMPOSE-001: Given P₁ → P₂ (P₂ has P₁ as ancestor), the composition
//! P₃ = P₁ ∘ P₂ satisfies:
//!   apply(P₃, pre_P₁_state) = apply(P₂, apply(P₁, pre_P₁_state))
//!
//! THM-COMPOSE-001: If P₁ and P₂ commute, then P₁ ∘ P₂ = P₂ ∘ P₁.
//!   (The composed patch is independent of ordering.)

use crate::patch::types::{OperationType, Patch};

/// Result of composing two patches.
#[derive(Clone, Debug)]
pub struct ComposeResult {
    /// The composed patch.
    pub patch: Patch,
    /// Number of patches that were composed.
    pub count: usize,
}

/// Compose two patches into a single equivalent patch.
///
/// The patches must form a chain: P₂ must have P₁ as its direct ancestor.
///
/// # Arguments
/// * `p1` - The first (earlier) patch
/// * `p2` - The second (later) patch, must have p1.id in parent_ids
/// * `author` - Author of the composition operation
/// * `message` - Description of the composition
///
/// # Returns
///
/// A ComposeResult with the composed patch, or an error if composition is not possible.
pub fn compose(
    p1: &Patch,
    p2: &Patch,
    author: &str,
    message: &str,
) -> Result<ComposeResult, ComposeError> {
    // Verify p2 has p1 as parent
    if !p2.parent_ids.contains(&p1.id) {
        return Err(ComposeError::NotAncestor {
            p1_id: p1.id.to_hex(),
            p2_id: p2.id.to_hex(),
        });
    }

    // Union of touch sets
    let mut composed_touch = p1.touch_set.clone();
    for addr in p2.touch_set.iter() {
        composed_touch.insert(addr.clone());
    }

    // If both modify the same file, use the later payload
    let composed_path = p2.target_path.clone().or_else(|| p1.target_path.clone());
    let composed_payload = if p2.payload.is_empty() {
        p1.payload.clone()
    } else {
        p2.payload.clone()
    };

    // Determine operation type:
    // - If p2 creates a file that p1 didn't touch → Create
    // - If p2 deletes a file → Delete
    // - If either modifies → Modify
    // - If p2 moves → Move
    let composed_op = match (&p1.operation_type, &p2.operation_type) {
        (_, OperationType::Delete) => OperationType::Delete,
        (_, OperationType::Move) => OperationType::Move,
        (_, OperationType::Create) => {
            if p1.operation_type == OperationType::Create {
                OperationType::Create
            } else {
                OperationType::Modify
            }
        }
        (OperationType::Create, OperationType::Modify) => OperationType::Create,
        _ => OperationType::Modify,
    };

    let composed_patch = Patch::new(
        composed_op,
        composed_touch,
        composed_path,
        composed_payload,
        p1.parent_ids.clone(), // Take p1's parents as the composed patch's parents
        author.to_owned(),
        message.to_owned(),
    );

    Ok(ComposeResult {
        patch: composed_patch,
        count: 2,
    })
}

/// Compose a sequence of patches into a single equivalent patch.
///
/// Patches must form a linear chain: each patch's first parent is the previous patch.
/// Unlike `compose`, this does not require strict ancestry verification between
/// intermediate composed results and the next patch, since the intermediate
/// composed patch has a new ID.
pub fn compose_chain(
    patches: &[Patch],
    author: &str,
    message: &str,
) -> Result<ComposeResult, ComposeError> {
    if patches.is_empty() {
        return Err(ComposeError::EmptyChain);
    }
    if patches.len() == 1 {
        return Ok(ComposeResult {
            patch: patches[0].clone(),
            count: 1,
        });
    }

    let mut composed_touch = patches[0].touch_set.clone();
    let mut composed_path = patches[0].target_path.clone();
    let mut composed_payload = patches[0].payload.clone();
    let mut composed_op = patches[0].operation_type;

    for p in &patches[1..] {
        for addr in p.touch_set.iter() {
            composed_touch.insert(addr.clone());
        }
        if !p.payload.is_empty() {
            composed_payload.clone_from(&p.payload);
        }
        if p.target_path.is_some() {
            composed_path.clone_from(&p.target_path);
        }
        composed_op = match (&composed_op, &p.operation_type) {
            (_, OperationType::Delete) => OperationType::Delete,
            (_, OperationType::Move) => OperationType::Move,
            (_, OperationType::Create) => {
                if composed_op == OperationType::Create {
                    OperationType::Create
                } else {
                    OperationType::Modify
                }
            }
            (OperationType::Create, OperationType::Modify) => OperationType::Create,
            _ => OperationType::Modify,
        };
    }

    let composed_patch = Patch::new(
        composed_op,
        composed_touch,
        composed_path,
        composed_payload,
        patches[0].parent_ids.clone(),
        author.to_owned(),
        message.to_owned(),
    );

    Ok(ComposeResult {
        patch: composed_patch,
        count: patches.len(),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    #[error("patches do not form a chain: {p2_id} does not have {p1_id} as ancestor")]
    NotAncestor { p1_id: String, p2_id: String },
    #[error("cannot compose an empty patch chain")]
    EmptyChain,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::types::{PatchId, TouchSet};
    use suture_common::Hash;

    fn make_patch(
        op: OperationType,
        touch: &[&str],
        path: Option<&str>,
        payload: &[u8],
        parents: &[PatchId],
        author: &str,
        message: &str,
    ) -> Patch {
        Patch::new(
            op,
            TouchSet::from_addrs(touch.iter().copied()),
            path.map(|s| s.to_string()),
            payload.to_vec(),
            parents.to_vec(),
            author.to_string(),
            message.to_string(),
        )
    }

    #[test]
    fn test_compose_linear_chain() {
        let root = Hash::from_data(b"root");
        let p1 = make_patch(
            OperationType::Modify,
            &["file_a"],
            Some("file_a"),
            b"content_a",
            &[root],
            "alice",
            "edit file_a",
        );
        let p2 = make_patch(
            OperationType::Modify,
            &["file_b"],
            Some("file_b"),
            b"content_b",
            &[p1.id],
            "alice",
            "edit file_b",
        );

        let result = compose(&p1, &p2, "alice", "composed").unwrap();
        assert_eq!(result.count, 2);
        assert_eq!(result.patch.parent_ids, vec![root]);
        assert!(result.patch.touch_set.contains("file_a"));
        assert!(result.patch.touch_set.contains("file_b"));
    }

    #[test]
    fn test_compose_disjoint_touch_sets() {
        let root = Hash::from_data(b"root");
        let p1 = make_patch(
            OperationType::Modify,
            &["alpha"],
            Some("alpha"),
            b"aaa",
            &[root],
            "bob",
            "change alpha",
        );
        let p2 = make_patch(
            OperationType::Modify,
            &["beta"],
            Some("beta"),
            b"bbb",
            &[p1.id],
            "bob",
            "change beta",
        );

        let result = compose(&p1, &p2, "bob", "merge disjoint").unwrap();
        assert_eq!(result.patch.touch_set.len(), 2);
        assert!(result.patch.touch_set.contains("alpha"));
        assert!(result.patch.touch_set.contains("beta"));
    }

    #[test]
    fn test_compose_overlapping_touch_sets() {
        let root = Hash::from_data(b"root");
        let p1 = make_patch(
            OperationType::Modify,
            &["shared"],
            Some("shared"),
            b"v1",
            &[root],
            "carol",
            "first edit",
        );
        let p2 = make_patch(
            OperationType::Modify,
            &["shared"],
            Some("shared"),
            b"v2",
            &[p1.id],
            "carol",
            "second edit",
        );

        let result = compose(&p1, &p2, "carol", "merge overlap").unwrap();
        // Touch set should have shared only (union of identical sets)
        assert_eq!(result.patch.touch_set.len(), 1);
        assert!(result.patch.touch_set.contains("shared"));
        // Should take p2's payload (later)
        assert_eq!(result.patch.payload, b"v2".to_vec());
    }

    #[test]
    fn test_compose_not_ancestor_error() {
        let root = Hash::from_data(b"root");
        let p1 = make_patch(
            OperationType::Modify,
            &["a"],
            Some("a"),
            b"aaa",
            &[root],
            "dave",
            "p1",
        );
        // p2 does NOT have p1 as ancestor — both share root
        let p2 = make_patch(
            OperationType::Modify,
            &["b"],
            Some("b"),
            b"bbb",
            &[root],
            "dave",
            "p2",
        );

        let err = compose(&p1, &p2, "dave", "fail").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("does not have"), "unexpected error: {msg}");
    }

    #[test]
    fn test_compose_empty_chain_error() {
        let err = compose_chain(&[], "alice", "empty").unwrap_err();
        assert!(matches!(err, ComposeError::EmptyChain));
    }

    #[test]
    fn test_compose_single_patch() {
        let root = Hash::from_data(b"root");
        let p1 = make_patch(
            OperationType::Modify,
            &["solo"],
            Some("solo"),
            b"data",
            &[root],
            "eve",
            "solo commit",
        );

        let result = compose_chain(&[p1.clone()], "eve", "noop").unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.patch.id, p1.id);
    }

    #[test]
    fn test_compose_chain_multiple() {
        let root = Hash::from_data(b"root");
        let p1 = make_patch(
            OperationType::Modify,
            &["x"],
            Some("x"),
            b"x1",
            &[root],
            "frank",
            "first",
        );
        let p2 = make_patch(
            OperationType::Modify,
            &["y"],
            Some("y"),
            b"y1",
            &[p1.id],
            "frank",
            "second",
        );
        let p3 = make_patch(
            OperationType::Modify,
            &["z"],
            Some("z"),
            b"z1",
            &[p2.id],
            "frank",
            "third",
        );

        let result = compose_chain(&[p1, p2, p3], "frank", "all three").unwrap();
        assert_eq!(result.count, 3);
        assert_eq!(result.patch.parent_ids, vec![root]);
        assert!(result.patch.touch_set.contains("x"));
        assert!(result.patch.touch_set.contains("y"));
        assert!(result.patch.touch_set.contains("z"));
    }

    #[test]
    fn test_compose_preserves_union_touch_set() {
        let root = Hash::from_data(b"root");
        let p1 = make_patch(
            OperationType::Modify,
            &["a", "b", "c"],
            Some("a"),
            b"data",
            &[root],
            "grace",
            "batch 1",
        );
        let p2 = make_patch(
            OperationType::Modify,
            &["c", "d", "e"],
            Some("d"),
            b"data2",
            &[p1.id],
            "grace",
            "batch 2",
        );

        let result = compose(&p1, &p2, "grace", "union test").unwrap();

        // Union should be {a, b, c, d, e}
        assert!(result.patch.touch_set.contains("a"));
        assert!(result.patch.touch_set.contains("b"));
        assert!(result.patch.touch_set.contains("c"));
        assert!(result.patch.touch_set.contains("d"));
        assert!(result.patch.touch_set.contains("e"));
        assert_eq!(result.patch.touch_set.len(), 5);
    }
}
