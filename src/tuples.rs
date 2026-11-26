// Simple mode: tuple types the program builds itself get a named record (`record Pos(int r,
// int c)`) instead of `Map.Entry` / `TupleN`. `Ty::java()` has no `Cx`, so the names live here,
// set per file by `lower::tuples` before lowering.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::ty::Ty;

#[derive(Clone, Debug)]
pub struct Named {
    pub name: String,
    pub fields: Vec<String>,
}

static NAMED: RwLock<Option<HashMap<String, Named>>> = RwLock::new(None);
// Simple mode on a one-file crate: a multi-file crate would need the records shared.
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn enable(on: bool) {
    ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

// The registry key: the components' structure, independent of how nested tuples are named.
pub fn key(ts: &[Ty]) -> String {
    let parts: Vec<String> = ts.iter().map(|t| format!("{:?}", t.strip_refs())).collect();
    parts.join(",")
}

pub fn set(map: HashMap<String, Named>) {
    *NAMED.write().unwrap() = Some(map);
}

pub fn clear() {
    *NAMED.write().unwrap() = None;
}

pub fn named(ts: &[Ty]) -> Option<Named> {
    let guard = NAMED.read().unwrap();
    guard.as_ref()?.get(&key(ts)).cloned()
}

// Every named record, sorted by name.
pub fn all() -> Vec<Named> {
    let guard = NAMED.read().unwrap();
    let mut v: Vec<Named> = guard
        .as_ref()
        .map(|m| m.values().cloned().collect())
        .unwrap_or_default();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

// The accessor method for field `i`: `r`, `getKey`, `_2`.
pub fn accessor(ts: &[Ty], i: usize) -> String {
    if let Some(n) = named(ts) {
        return n.fields[i].clone();
    }
    match (ts.len(), i) {
        (2, 0) => "getKey".into(),
        (2, _) => "getValue".into(),
        _ => format!("_{i}"),
    }
}
