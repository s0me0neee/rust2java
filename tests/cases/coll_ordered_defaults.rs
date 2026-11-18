// Ordered collections of Vec/tuple keys from derive(Default), or_default() and Default::default().
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
struct Graph {
    edges: BTreeSet<(u32, u32)>,
    paths: BTreeMap<Vec<u8>, usize>,
    name: String,
}
fn main() {
    let mut g = Graph::default();
    g.edges.insert((2, 1));
    g.edges.insert((1, 5));
    g.paths.insert(vec![2], 1);
    g.paths.insert(vec![1, 9], 2);
    let mut groups: BTreeMap<u8, BTreeSet<Vec<i32>>> = BTreeMap::new();
    groups.entry(1).or_default().insert(vec![3]);
    groups.entry(1).or_default().insert(vec![1, 2]);
    let empty: BTreeSet<(i32, String)> = Default::default();
    println!("{:?} {:?} {:?} {:?} {}", g.edges, g.paths, groups, empty, g.name.len());
}
