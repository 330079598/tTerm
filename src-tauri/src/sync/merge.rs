//! Three-way merge of synced collections.
//!
//! Every synced category is a collection of records keyed by a stable id.
//! Merging compares the local and remote versions of each record with the
//! version both sides had at the last sync (the base), so a change made on
//! only one side is taken, deletions travel, and only records changed on
//! both sides need a rule: their top-level fields are merged the same way,
//! and a field changed differently on both sides takes the side whose record
//! was modified later (the remote side on a tie or without timestamps).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncRecord {
    pub data: Value,
    /// Unix milliseconds of the last change, when the store records one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
}

impl SyncRecord {
    pub fn new(data: Value, updated_at: Option<i64>) -> Self {
        Self { data, updated_at }
    }
}

pub(crate) type Collection = BTreeMap<String, SyncRecord>;

fn data(record: Option<&SyncRecord>) -> Option<&Value> {
    record.map(|record| &record.data)
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct MergeOutcome {
    pub merged: Collection,
    /// Ids of records changed differently on both sides.
    pub conflicts: Vec<String>,
}

/// Merges one collection. `base` is `None` before the first sync, which makes
/// the merge a union: nothing can have been deleted since a base that never
/// existed.
pub(crate) fn merge_collection(
    base: Option<&Collection>,
    local: &Collection,
    remote: &Collection,
) -> MergeOutcome {
    let empty = Collection::new();
    let base = base.unwrap_or(&empty);
    let ids: BTreeSet<&String> = base
        .keys()
        .chain(local.keys())
        .chain(remote.keys())
        .collect();
    let mut outcome = MergeOutcome::default();
    for id in ids {
        let (b, l, r) = (base.get(id), local.get(id), remote.get(id));
        let merged = if data(l) == data(r) {
            newer(l, r).cloned()
        } else if data(l) == data(b) {
            r.cloned()
        } else if data(r) == data(b) {
            l.cloned()
        } else {
            outcome.conflicts.push(id.clone());
            match (l, r) {
                // An edit wins over a deletion: losing the edit loses data,
                // restoring a deleted record does not.
                (Some(record), None) | (None, Some(record)) => Some(record.clone()),
                (Some(l), Some(r)) => Some(merge_record(b, l, r)),
                (None, None) => None,
            }
        };
        if let Some(record) = merged {
            outcome.merged.insert(id.clone(), record);
        }
    }
    outcome
}

/// The later of two records with equal data, so the merged record keeps the
/// most recent timestamp either side knows.
fn newer<'a>(l: Option<&'a SyncRecord>, r: Option<&'a SyncRecord>) -> Option<&'a SyncRecord> {
    match (l, r) {
        (Some(l), Some(r)) if l.updated_at > r.updated_at => Some(l),
        (_, Some(r)) => Some(r),
        (l, None) => l,
    }
}

/// Whether the local record wins a field both sides changed.
fn local_wins(l: &SyncRecord, r: &SyncRecord) -> bool {
    matches!((l.updated_at, r.updated_at), (Some(l), Some(r)) if l > r)
}

/// Merges a record changed on both sides field by field.
fn merge_record(base: Option<&SyncRecord>, l: &SyncRecord, r: &SyncRecord) -> SyncRecord {
    let updated_at = l.updated_at.max(r.updated_at);
    let (Value::Object(lf), Value::Object(rf)) = (&l.data, &r.data) else {
        let winner = if local_wins(l, r) { l } else { r };
        return SyncRecord::new(winner.data.clone(), updated_at);
    };
    let empty = Map::new();
    let bf = match base.map(|record| &record.data) {
        Some(Value::Object(fields)) => fields,
        _ => &empty,
    };
    let keys: BTreeSet<&String> = bf.keys().chain(lf.keys()).chain(rf.keys()).collect();
    let mut fields = Map::new();
    for key in keys {
        let (b, lv, rv) = (bf.get(key), lf.get(key), rf.get(key));
        let value = if lv == rv || lv == b {
            rv
        } else if rv == b || local_wins(l, r) {
            lv
        } else {
            rv
        };
        if let Some(value) = value {
            fields.insert(key.clone(), value.clone());
        }
    }
    SyncRecord::new(Value::Object(fields), updated_at)
}

/// Whether two collections hold the same data, ignoring timestamps.
pub(crate) fn same_data(left: &Collection, right: &Collection) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((lk, lv), (rk, rv))| lk == rk && lv.data == rv.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(data: Value, updated_at: Option<i64>) -> SyncRecord {
        SyncRecord::new(data, updated_at)
    }

    fn collection(items: &[(&str, Value, Option<i64>)]) -> Collection {
        items
            .iter()
            .map(|(id, data, at)| (id.to_string(), record(data.clone(), *at)))
            .collect()
    }

    fn data(collection: &Collection) -> BTreeMap<&str, &Value> {
        collection
            .iter()
            .map(|(id, r)| (id.as_str(), &r.data))
            .collect()
    }

    #[test]
    fn takes_one_sided_changes_additions_and_deletions() {
        let base = collection(&[
            ("same", json!(1), None),
            ("local-edit", json!(1), None),
            ("remote-edit", json!(1), None),
            ("local-delete", json!(1), None),
            ("remote-delete", json!(1), None),
            ("both-delete", json!(1), None),
        ]);
        let local = collection(&[
            ("same", json!(1), None),
            ("local-edit", json!(2), None),
            ("remote-edit", json!(1), None),
            ("remote-delete", json!(1), None),
            ("local-add", json!(5), None),
        ]);
        let remote = collection(&[
            ("same", json!(1), None),
            ("local-edit", json!(1), None),
            ("remote-edit", json!(3), None),
            ("local-delete", json!(1), None),
            ("remote-add", json!(6), None),
        ]);
        let outcome = merge_collection(Some(&base), &local, &remote);
        assert!(outcome.conflicts.is_empty());
        assert_eq!(
            data(&outcome.merged),
            BTreeMap::from([
                ("same", &json!(1)),
                ("local-edit", &json!(2)),
                ("remote-edit", &json!(3)),
                ("local-add", &json!(5)),
                ("remote-add", &json!(6)),
            ])
        );
    }

    #[test]
    fn first_sync_is_a_union() {
        let local = collection(&[
            ("a", json!({"x": 1}), None),
            ("both", json!({"x": 1}), None),
        ]);
        let remote = collection(&[
            ("b", json!({"x": 2}), None),
            ("both", json!({"x": 1}), None),
        ]);
        let outcome = merge_collection(None, &local, &remote);
        assert!(outcome.conflicts.is_empty());
        assert_eq!(outcome.merged.len(), 3);
    }

    #[test]
    fn merges_different_fields_changed_on_both_sides() {
        let base = collection(&[("p", json!({"host": "a", "port": 22, "group": "x"}), Some(1))]);
        let local = collection(&[("p", json!({"host": "b", "port": 22, "group": "x"}), Some(5))]);
        let remote = collection(&[(
            "p",
            json!({"host": "a", "port": 2222, "group": "x", "added": true}),
            Some(3),
        )]);
        let outcome = merge_collection(Some(&base), &local, &remote);
        assert_eq!(outcome.conflicts, ["p"]);
        assert_eq!(
            outcome.merged["p"],
            record(
                json!({"host": "b", "port": 2222, "group": "x", "added": true}),
                Some(5)
            )
        );
    }

    #[test]
    fn same_field_conflicts_take_the_later_change_and_remote_on_ties() {
        let base = collection(&[("p", json!({"host": "a"}), Some(1))]);
        let local = |at| collection(&[("p", json!({"host": "local"}), at)]);
        let remote = |at| collection(&[("p", json!({"host": "remote"}), at)]);
        let host = |outcome: MergeOutcome| outcome.merged["p"].data["host"].clone();
        assert_eq!(
            host(merge_collection(
                Some(&base),
                &local(Some(9)),
                &remote(Some(5))
            )),
            "local"
        );
        assert_eq!(
            host(merge_collection(
                Some(&base),
                &local(Some(5)),
                &remote(Some(9))
            )),
            "remote"
        );
        assert_eq!(
            host(merge_collection(
                Some(&base),
                &local(Some(5)),
                &remote(Some(5))
            )),
            "remote"
        );
        assert_eq!(
            host(merge_collection(Some(&base), &local(None), &remote(None))),
            "remote"
        );
        assert_eq!(
            host(merge_collection(
                Some(&base),
                &local(Some(9)),
                &remote(None)
            )),
            "remote"
        );
    }

    #[test]
    fn a_field_removed_on_one_side_stays_removed() {
        let base = collection(&[("p", json!({"a": 1, "b": 1}), None)]);
        let local = collection(&[("p", json!({"a": 2, "b": 1}), None)]);
        let remote = collection(&[("p", json!({"a": 1}), None)]);
        let merged = merge_collection(Some(&base), &local, &remote).merged;
        assert_eq!(merged["p"].data, json!({"a": 2}));
    }

    #[test]
    fn an_edit_wins_over_a_deletion() {
        let base = collection(&[("p", json!(1), None), ("q", json!(1), None)]);
        let local = collection(&[("p", json!(2), None)]);
        let remote = collection(&[("q", json!(3), None)]);
        let outcome = merge_collection(Some(&base), &local, &remote);
        assert_eq!(
            data(&outcome.merged),
            BTreeMap::from([("p", &json!(2)), ("q", &json!(3))])
        );
        assert_eq!(outcome.conflicts, ["p", "q"]);
    }

    #[test]
    fn non_object_conflicts_pick_a_whole_side() {
        let base = collection(&[("v", json!([1]), Some(1))]);
        let local = collection(&[("v", json!([1, 2]), Some(4))]);
        let remote = collection(&[("v", json!([1, 3]), Some(2))]);
        let merged = merge_collection(Some(&base), &local, &remote).merged;
        assert_eq!(merged["v"], record(json!([1, 2]), Some(4)));
    }

    #[test]
    fn equal_changes_keep_the_newest_timestamp() {
        let base = collection(&[("p", json!(1), Some(1))]);
        let local = collection(&[("p", json!(2), Some(7))]);
        let remote = collection(&[("p", json!(2), Some(3))]);
        let outcome = merge_collection(Some(&base), &local, &remote);
        assert!(outcome.conflicts.is_empty());
        assert_eq!(outcome.merged["p"], record(json!(2), Some(7)));
    }

    #[test]
    fn merging_is_stable_once_both_sides_agree() {
        let base = collection(&[("p", json!({"a": 1}), Some(1))]);
        let local = collection(&[("p", json!({"a": 2}), Some(2)), ("n", json!(1), None)]);
        let remote = collection(&[("p", json!({"a": 1}), Some(1)), ("m", json!(1), None)]);
        let merged = merge_collection(Some(&base), &local, &remote).merged;
        let again = merge_collection(Some(&merged), &merged, &merged);
        assert_eq!(again.merged, merged);
        assert!(same_data(&merged, &again.merged));
    }

    #[test]
    fn compares_data_without_timestamps() {
        let left = collection(&[("a", json!(1), Some(1))]);
        assert!(same_data(&left, &collection(&[("a", json!(1), Some(2))])));
        assert!(!same_data(&left, &collection(&[("a", json!(2), Some(1))])));
        assert!(!same_data(&left, &collection(&[("b", json!(1), Some(1))])));
        assert!(!same_data(&left, &Collection::new()));
    }
}
