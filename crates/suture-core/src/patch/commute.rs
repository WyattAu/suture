//! Commutativity checking for patches.
//!
//! Two patches commute if they can be applied in either order and produce
//! the same result. In Suture, commutativity is determined by touch-set
//! disjointness (THM-COMM-001 from YP-ALGEBRA-PATCH-001).

use crate::patch::types::Patch;

/// Result of a commutativity check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommuteResult {
    /// The two patches commute — they can be applied in any order.
    Commutes,
    /// The two patches do NOT commute — their touch sets overlap.
    DoesNotCommute {
        /// The addresses where both patches touch (the conflict addresses).
        conflict_addresses: Vec<String>,
    },
}

/// Check if two patches commute.
///
/// # Correctness
///
/// Per THM-COMM-001 (YP-ALGEBRA-PATCH-001):
/// If T(P₁) ∩ T(P₂) = ∅, then P₁ ∘ P₂ = P₂ ∘ P₁.
///
/// This function implements the sufficient condition: disjoint touch sets.
/// Note that some patches with overlapping touch sets MAY still commute
/// (e.g., writing the same value), but we conservatively report them as
/// non-commuting to guarantee correctness.
pub fn commute(p1: &Patch, p2: &Patch) -> CommuteResult {
    // Identity patches commute with everything
    if p1.is_identity() || p2.is_identity() {
        return CommuteResult::Commutes;
    }

    if !p1.touch_set.intersects(&p2.touch_set) {
        CommuteResult::Commutes
    } else {
        let intersection = p1.touch_set.intersection(&p2.touch_set);
        let conflict_addresses: Vec<String> = intersection.iter().cloned().collect();
        CommuteResult::DoesNotCommute { conflict_addresses }
    }
}

/// Check if a list of patches are pairwise commutative.
///
/// Returns `true` only if ALL pairs in the list commute.
/// This is O(n²) in the number of patches.
#[allow(dead_code)]
pub fn all_commute(patches: &[Patch]) -> bool {
    for i in 0..patches.len() {
        for j in (i + 1)..patches.len() {
            if !matches!(commute(&patches[i], &patches[j]), CommuteResult::Commutes) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::types::{OperationType, Patch, TouchSet};
    use suture_common::Hash;

    fn patch_with_touch(addr: &str) -> Patch {
        Patch::new(
            OperationType::Modify,
            TouchSet::single(addr),
            Some(format!("file_{}", addr)),
            vec![],
            vec![],
            "test".to_string(),
            format!("edit {}", addr),
        )
    }

    fn patch_with_touches(addrs: &[&str]) -> Patch {
        Patch::new(
            OperationType::Modify,
            TouchSet::from_addrs(addrs.iter().copied()),
            None,
            vec![],
            vec![],
            "test".to_string(),
            "multi edit".to_string(),
        )
    }

    #[test]
    fn test_disjoint_patches_commute() {
        let p1 = patch_with_touch("A1");
        let p2 = patch_with_touch("B1");
        assert_eq!(commute(&p1, &p2), CommuteResult::Commutes);
    }

    #[test]
    fn test_overlapping_patches_do_not_commute() {
        let p1 = patch_with_touch("A1");
        let p2 = patch_with_touch("A1");
        assert_eq!(
            commute(&p1, &p2),
            CommuteResult::DoesNotCommute {
                conflict_addresses: vec!["A1".to_string()]
            }
        );
    }

    #[test]
    fn test_identity_commutes_with_everything() {
        let parent = Hash::ZERO;
        let identity = Patch::identity(parent, "test".to_string());
        let p = patch_with_touch("A1");
        assert_eq!(commute(&identity, &p), CommuteResult::Commutes);
        assert_eq!(commute(&p, &identity), CommuteResult::Commutes);
    }

    #[test]
    fn test_partial_overlap() {
        let p1 = patch_with_touches(&["A1", "B1", "C1"]);
        let p2 = patch_with_touches(&["C1", "D1", "E1"]);

        match commute(&p1, &p2) {
            CommuteResult::DoesNotCommute { conflict_addresses } => {
                assert_eq!(conflict_addresses, vec!["C1".to_string()]);
            }
            CommuteResult::Commutes => panic!("Expected DoesNotCommute"),
        }
    }

    #[test]
    fn test_commutativity_is_symmetric() {
        let p1 = patch_with_touch("A1");
        let p2 = patch_with_touch("B1");
        assert_eq!(commute(&p1, &p2), commute(&p2, &p1));
    }

    #[test]
    fn test_all_commute_empty() {
        assert!(all_commute(&[]));
    }

    #[test]
    fn test_all_commute_single() {
        let p = patch_with_touch("A1");
        assert!(all_commute(&[p]));
    }

    #[test]
    fn test_all_commute_disjoint() {
        let patches = vec![
            patch_with_touch("A1"),
            patch_with_touch("B1"),
            patch_with_touch("C1"),
        ];
        assert!(all_commute(&patches));
    }

    #[test]
    fn test_all_commute_with_overlap() {
        let patches = vec![
            patch_with_touch("A1"),
            patch_with_touch("B1"),
            patch_with_touch("A1"), // Overlaps with first
        ];
        assert!(!all_commute(&patches));
    }

    mod proptests {
        use super::*;
        use crate::patch::types::{OperationType, Patch, TouchSet};
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn commute_commutativity_class_symmetric(
                addrs1 in proptest::collection::vec("[a-z]{1,5}", 0..10),
                addrs2 in proptest::collection::vec("[a-z]{1,5}", 0..10),
            ) {
                let p1 = Patch::new(
                    OperationType::Modify,
                    TouchSet::from_addrs(addrs1.iter()),
                    None,
                    vec![],
                    vec![],
                    "proptest".to_string(),
                    "patch1".to_string(),
                );
                let p2 = Patch::new(
                    OperationType::Modify,
                    TouchSet::from_addrs(addrs2.iter()),
                    None,
                    vec![],
                    vec![],
                    "proptest".to_string(),
                    "patch2".to_string(),
                );
                let r1 = commute(&p1, &p2);
                let r2 = commute(&p2, &p1);
                match (&r1, &r2) {
                    (CommuteResult::Commutes, CommuteResult::Commutes) => {},
                    (CommuteResult::DoesNotCommute { conflict_addresses: c1 },
                     CommuteResult::DoesNotCommute { conflict_addresses: c2 }) => {
                        let mut s1: Vec<_> = c1.clone();
                        let mut s2: Vec<_> = c2.clone();
                        s1.sort();
                        s2.sort();
                        prop_assert_eq!(s1, s2, "conflict addresses should match");
                    }
                    _ => prop_assert!(false, "commute results should both be Commutes or both DoesNotCommute: {:?} vs {:?}", r1, r2),
                }
            }

            #[test]
            fn disjoint_always_commutes(
                addrs1 in proptest::collection::vec("[a-z]{1,5}", 0..10),
                addrs2 in proptest::collection::vec("[a-z]{1,5}", 0..10),
            ) {
                let set1: std::collections::HashSet<_> = addrs1.iter().collect();
                let set2: std::collections::HashSet<_> = addrs2.iter().collect();
                prop_assume!(set1.is_disjoint(&set2));

                let p1 = Patch::new(
                    OperationType::Modify,
                    TouchSet::from_addrs(addrs1.iter()),
                    None,
                    vec![],
                    vec![],
                    "proptest".to_string(),
                    "p1".to_string(),
                );
                let p2 = Patch::new(
                    OperationType::Modify,
                    TouchSet::from_addrs(addrs2.iter()),
                    None,
                    vec![],
                    vec![],
                    "proptest".to_string(),
                    "p2".to_string(),
                );
                prop_assert_eq!(commute(&p1, &p2), CommuteResult::Commutes);
            }

            #[test]
            fn identity_commutes_with_any(
                addrs in proptest::collection::vec("[a-z]{1,5}", 0..10),
            ) {
                let parent = Hash::from_data(b"proptest-parent");
                let identity = Patch::identity(parent, "proptest".to_string());
                let p = Patch::new(
                    OperationType::Modify,
                    TouchSet::from_addrs(addrs.iter()),
                    None,
                    vec![],
                    vec![],
                    "proptest".to_string(),
                    "p".to_string(),
                );
                prop_assert_eq!(commute(&identity, &p), CommuteResult::Commutes);
                prop_assert_eq!(commute(&p, &identity), CommuteResult::Commutes);
            }
        }
    }
}
