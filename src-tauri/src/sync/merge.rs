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
mod tests;
