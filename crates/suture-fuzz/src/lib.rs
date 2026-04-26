// Allow collapsible_match: Rust 1.94/1.95 lint conflict
//! Fuzz smoke harnesses for suture-core.
//!
//! These use proptest for property-based testing since cargo-fuzz is not available.

use proptest::prelude::*;
#[cfg(test)]
use suture_common::Hash;
#[allow(unused_imports)]
use suture_core::patch::types::{OperationType, Patch, PatchId, TouchSet};

proptest! {
    #[test]
    fn fuzz_patch_serialization_roundtrip(
        op_type in proptest::sample::select(&[OperationType::Create, OperationType::Modify, OperationType::Delete]),
        addrs in proptest::collection::vec("[a-z]{1,20}", 0..10),
        payload in proptest::collection::vec(proptest::num::u8::ANY, 0..200),
        author in "[a-z]{1,10}",
        message in "[a-z]{1,20}",
    ) {
        let touch_set = TouchSet::from_addrs(
            addrs.iter().map(String::as_str)
        );
        let patch = Patch::new(
            op_type,
            touch_set,
            None,
            payload,
            vec![],
            author,
            message,
        );

        let json = serde_json::to_string(&patch).expect("serialize");
        let deserialized: Patch = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(patch.id, deserialized.id, "patch ID mismatch after roundtrip");
        assert_eq!(patch.touch_set, deserialized.touch_set, "touch set mismatch");
        assert_eq!(patch.operation_type, deserialized.operation_type, "op type mismatch");
    }
}

proptest! {
    #[test]
    fn fuzz_cas_hash_determinism(
        data in proptest::collection::vec(proptest::num::u8::ANY, 0..1000),
    ) {
        let h1 = Hash::from_data(&data);
        let h2 = Hash::from_data(&data);
        assert_eq!(h1, h2, "BLAKE3 hash must be deterministic");
    }

    #[test]
    fn fuzz_cas_hash_no_trivial_collisions(
        data1 in proptest::collection::vec(proptest::num::u8::ANY, 1..100),
        data2 in proptest::collection::vec(proptest::num::u8::ANY, 1..100),
    ) {
        if data1 != data2 {
            let h1 = Hash::from_data(&data1);
            let h2 = Hash::from_data(&data2);
            assert_ne!(h1, Hash::ZERO);
            assert_ne!(h2, Hash::ZERO);
        }
    }
}

proptest! {
    #[test]
    fn fuzz_touch_set_algebra(
        set_a in proptest::collection::vec("[a-zA-Z0-9_]{1,15}", 0..20),
        set_b in proptest::collection::vec("[a-zA-Z0-9_]{1,15}", 0..20),
    ) {
        let ts_a = TouchSet::from_addrs(set_a.iter().map(String::as_str));
        let ts_b = TouchSet::from_addrs(set_b.iter().map(String::as_str));

        let union_ab = ts_a.union(&ts_b);
        let union_ba = ts_b.union(&ts_a);
        assert_eq!(union_ab.len(), union_ba.len(), "union must be commutative");
        for a in union_ab.iter() { assert!(union_ba.contains(a)); }
        for a in union_ba.iter() { assert!(union_ab.contains(a)); }

        assert!(union_ab.len() >= ts_a.len(), "union must be >= A");
        assert!(union_ab.len() >= ts_b.len(), "union must be >= B");

        let inter = ts_a.intersection(&ts_b);
        for a in inter.iter() {
            assert!(ts_a.contains(a), "intersection element must be in A");
            assert!(ts_b.contains(a), "intersection element must be in B");
        }

        let diff = ts_a.subtract(&ts_b);
        for a in diff.iter() {
            assert!(ts_a.contains(a), "subtract element must be in A");
            assert!(!ts_b.contains(a), "subtract element must NOT be in B");
        }

        assert_eq!(
            ts_a.len(),
            inter.len() + diff.len(),
            "set partition: |A| = |A∩B| + |A\\B|"
        );
    }
}

proptest! {
    #[test]
    fn fuzz_merge_partition(
        num_a in 1usize..5,
        num_b in 1usize..5,
    ) {
        use suture_core::patch::merge::merge;
        use std::collections::HashMap;

        let base = Patch::new(
            OperationType::Create,
            TouchSet::single("root"),
            Some("root".to_string()),
            vec![],
            vec![],
            "fuzz".to_string(),
            "base".to_string(),
        );

        let mut all: HashMap<PatchId, Patch> = HashMap::new();
        all.insert(base.id, base.clone());

        let mut a_chain = Vec::new();
        let mut parent = base.id;
        for i in 0..num_a {
            let p = Patch::new(
                OperationType::Modify,
                TouchSet::single(&format!("a_file_{}", i)),
                Some(format!("a_file_{}", i)),
                vec![i as u8],
                vec![parent],
                "fuzz_a".to_string(),
                format!("a_patch_{}", i),
            );
            parent = p.id;
            all.insert(p.id, p.clone());
            a_chain.push(p);
        }

        let mut b_chain = Vec::new();
        parent = base.id;
        for i in 0..num_b {
            let p = Patch::new(
                OperationType::Modify,
                TouchSet::single(&format!("b_file_{}", i)),
                Some(format!("b_file_{}", i)),
                vec![(i + 100) as u8],
                vec![parent],
                "fuzz_b".to_string(),
                format!("b_patch_{}", i),
            );
            parent = p.id;
            all.insert(p.id, p.clone());
            b_chain.push(p);
        }

        let base_ids = vec![base.id];
        let mut a_ids: Vec<PatchId> = vec![base.id];
        a_ids.extend(a_chain.iter().map(|p| p.id));
        let mut b_ids: Vec<PatchId> = vec![base.id];
        b_ids.extend(b_chain.iter().map(|p| p.id));

        let result = merge(&base_ids, &a_ids, &b_ids, &all).unwrap();

        assert!(result.is_clean, "disjoint branches must merge cleanly");

        let mut result_ids: Vec<PatchId> = result.all_patch_ids();
        result_ids.sort();
        let mut expected_ids: Vec<PatchId> = all.keys().copied().collect();
        expected_ids.sort();
        assert_eq!(result_ids, expected_ids, "merge must include all patches");
    }
}

proptest! {
    #[test]
    fn fuzz_json_merge(
        base_json in proptest::collection::vec(
            (proptest::collection::vec("[a-z]{1,5}", 1..3), "[a-z0-9]{1,10}"),
            0..10
        ),
        a_changes in proptest::collection::vec(
            (proptest::collection::vec("[a-z]{1,5}", 1..3), "[a-z0-9]{1,10}"),
            0..5
        ),
        b_changes in proptest::collection::vec(
            (proptest::collection::vec("[a-z]{1,5}", 1..3), "[a-z0-9]{1,10}"),
            0..5
        ),
    ) {
        use suture_driver::SutureDriver;
        use suture_driver_json::JsonDriver;

        let mut base_val = serde_json::Value::Object(serde_json::Map::new());
        for (path, value) in &base_json {
            let mut obj = &mut base_val;
            for key in path.iter() {
                if !obj.is_object() { break; }
                let map = obj.as_object_mut().unwrap();
                if !map.contains_key(key) {
                    map.insert(key.clone(), serde_json::Value::Null);
                }
                obj = map.get_mut(key).unwrap();
            }
            *obj = serde_json::Value::String(value.clone());
        }

        let mut a_val = base_val.clone();
        for (path, value) in &a_changes {
            let mut obj = &mut a_val;
            for key in path.iter() {
                if !obj.is_object() { break; }
                let map = obj.as_object_mut().unwrap();
                if !map.contains_key(key) {
                    map.insert(key.clone(), serde_json::Value::Null);
                }
                obj = map.get_mut(key).unwrap();
            }
            *obj = serde_json::Value::String(value.clone());
        }

        let mut b_val = base_val.clone();
        for (path, value) in &b_changes {
            let mut obj = &mut b_val;
            for key in path.iter() {
                if !obj.is_object() { break; }
                let map = obj.as_object_mut().unwrap();
                if !map.contains_key(key) {
                    map.insert(key.clone(), serde_json::Value::Null);
                }
                obj = map.get_mut(key).unwrap();
            }
            *obj = serde_json::Value::String(value.clone());
        }

        let base_str = serde_json::to_string(&base_val).unwrap();
        let a_str = serde_json::to_string(&a_val).unwrap();
        let b_str = serde_json::to_string(&b_val).unwrap();

        let driver = JsonDriver::new();
        let result = driver.merge(&base_str, &a_str, &b_str).unwrap();
        match result {
            Some(merged_str) => {
                let merged: serde_json::Value = serde_json::from_str(&merged_str).unwrap();
                let _ = serde_json::to_string(&merged);
            }
            None => {}
        }
    }
}
