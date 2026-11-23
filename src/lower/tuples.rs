// Simple mode: named records for the tuple types a program builds itself (DESIGN §22.7).
// A type is named only when nothing makes or takes it as a `Map.Entry`: map items, `enumerate`,
// `zip`, `split_once`, ... produce entries, so those pair types stay as they are.
// The record's name comes from a `type` alias or the bindings that hold it (`start` -> `Start`,
// `edges: Vec<..>` -> `Edge`); its fields from the patterns that unpack it (`let (r, c) = p`).

use std::collections::{HashMap, HashSet};

use ra_ap_syntax::{AstNode, SyntaxNode, ast, ast::HasName};

use super::Cx;
use crate::tuples::{Named, key};
use crate::ty::Ty;

// Methods whose result pairs up values that weren't a tuple before (lowered as `Map.entry`).
const PRODUCERS: &[&str] = &[
    "enumerate", "zip", "char_indices", "partition", "split_once", "rsplit_once", "split_at",
    "split_at_mut", "split_first", "split_last", "split_first_mut", "split_last_mut", "overflowing_add", "overflowing_sub", "overflowing_mul", "overflowing_neg",
    "overflowing_pow", "unzip",
];

// Binding names that say nothing about what the tuple is.
const VAGUE: &[&str] = &[
    "tmp", "item", "it", "val", "value", "res", "result", "cur", "curr", "current", "next",
    "prev", "top", "front", "back", "last", "first", "best", "out", "entry", "elem", "data",
    "pair", "tuple", "x", "y", "a", "b",
];

// Names records can't use for a component (`Object` methods) or that the file already means.
const RESERVED_FIELDS: &[&str] =
    &["clone", "finalize", "getClass", "hashCode", "notify", "notifyAll", "toString", "wait"];

pub fn analyze(cx: &mut Cx, root: &SyntaxNode) {
    let nodes = cx.expanded_descendants(root);
    let mut order: Vec<Vec<Ty>> = Vec::new();
    let mut seen = HashSet::new();
    let mut excluded = HashSet::new();
    let mut add = |ts: &[Ty], order: &mut Vec<Vec<Ty>>| {
        if seen.insert(key(ts)) {
            order.push(ts.to_vec());
        }
    };
    for n in &nodes {
        // `(String, Cell<usize>)`: the cell is a Java array where it's stored, not an `int`.
        if let Some(tt) = ast::TupleType::cast(n.clone())
            && tt.fields().any(|f| f.syntax().text().to_string().contains("Cell"))
        {
            walk(&cx.type_ref(&ast::Type::TupleType(tt)), &mut |ts| {
                excluded.insert(key(ts));
            });
        }
        let ty = if let Some(e) = ast::Expr::cast(n.clone()) {
            let t = cx.ty(&e);
            if let ast::Expr::MethodCallExpr(m) = &e {
                let name = super::iter::method_name(m);
                if PRODUCERS.contains(&name.as_str()) {
                    excluded.extend(produced(&t));
                    // `unzip` takes the pairs apart as entries.
                    if name == "unzip"
                        && let Some(r) = m.receiver()
                    {
                        excluded.extend(produced(&cx.ty(&r)));
                    }
                }
            }
            t
        } else if let Some(p) = ast::Pat::cast(n.clone()) {
            cx.pat_ty(&p)
        } else {
            continue;
        };
        // A map's items are its `Map.Entry`s.
        each_ty(&ty, &mut |t| {
            if let Ty::Map { k, v, .. } = t {
                excluded.insert(key(&[(**k).clone(), (**v).clone()]));
            }
        });
        walk(&ty, &mut |ts| add(ts, &mut order));
    }
    let names = Hints::collect(cx, root, &nodes);
    let mut taken: HashSet<String> = [
        "Main", "List", "Map", "Set", "Optional", "String", "Integer", "Long", "Double", "Character",
        "Boolean", "Object", "Math", "System", "Arrays", "Collections", "Stream", "IntStream",
        "Entry", "Iterator", "Comparator", "Record", "Result", "Ok", "Err",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for item in root.descendants() {
        let name = ast::Struct::cast(item.clone())
            .and_then(|s| s.name())
            .or_else(|| ast::Enum::cast(item.clone()).and_then(|e| e.name()))
            .or_else(|| ast::Trait::cast(item.clone()).and_then(|t| t.name()));
        if let Some(n) = name {
            taken.insert(n.text().to_string());
        }
    }
    let mut map = HashMap::new();
    let mut decls = Vec::new();
    for ts in order {
        let k = key(&ts);
        let generic = ts.iter().any(|t| !t.is_nameable() || has_param(t) || t.is_unit());
        if ts.len() < 2 || generic || excluded.contains(&k) {
            continue;
        }
        let base = names.record_name(&k).unwrap_or_else(|| {
            match ts.len() {
                2 => "Pair",
                3 => "Triple",
                _ => "Tuple",
            }
            .to_string()
        });
        let mut name = base.clone();
        let mut n = 2;
        while taken.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        taken.insert(name.clone());
        let fields = names.field_names(&k, ts.len());
        decls.push((name.clone(), ts.clone(), fields.clone()));
        map.insert(k, Named { name, fields });
    }
    crate::tuples::set(map);
    // Component types may name other records, so they're spelled once all names are set.
    for (name, ts, fields) in decls {
        let comps: Vec<String> = ts
            .iter()
            .zip(&fields)
            .map(|(t, f)| format!("{} {f}", t.strip_refs().java()))
            .collect();
        cx.file_trailer.insert(0, format!("record {name}({}) {{}}", comps.join(", ")));
    }
}

// The pair a producer makes: the tuple itself, or its iterator's / option's item. Tuples inside
// it (`(i, &(a, b, c))`) are the program's own.
fn produced(t: &Ty) -> Vec<String> {
    match t.strip_refs() {
        Ty::Iter(x) | Ty::Option(x) | Ty::Vec(x) => produced(x),
        Ty::Tuple(ts) if ts.len() >= 2 => {
            let ts: Vec<Ty> = ts.iter().map(|t| t.strip_refs().clone()).collect();
            vec![key(&ts)]
        }
        _ => Vec::new(),
    }
}

// Every tuple type inside `t` (itself included), innermost last.
fn walk(t: &Ty, f: &mut dyn FnMut(&[Ty])) {
    each_ty(t, &mut |x| {
        if let Ty::Tuple(ts) = x {
            let stripped: Vec<Ty> = ts.iter().map(|t| t.strip_refs().clone()).collect();
            if stripped.len() >= 2 {
                f(&stripped);
            }
        }
    });
}

fn each_ty(t: &Ty, f: &mut dyn FnMut(&Ty)) {
    f(t);
    match t {
        Ty::Vec(x) | Ty::Set { t: x, .. } | Ty::Deque(x) | Ty::Heap(x) | Ty::Option(x) | Ty::Iter(x) => {
            each_ty(x, f)
        }
        Ty::Map { k, v, .. } | Ty::Result(k, v) => {
            each_ty(k, f);
            each_ty(v, f);
        }
        Ty::Ref { inner, .. } => each_ty(inner, f),
        Ty::Tuple(ts) => ts.iter().for_each(|x| each_ty(x, f)),
        Ty::Adt { args, .. } => args.iter().for_each(|x| each_ty(x, f)),
        Ty::Fn { params, ret } => {
            params.iter().for_each(|x| each_ty(x, f));
            each_ty(ret, f);
        }
        _ => {}
    }
}

fn has_param(t: &Ty) -> bool {
    let mut found = false;
    each_ty(t, &mut |x| found |= matches!(x, Ty::Param(_) | Ty::Other(_)));
    found
}

#[derive(Default)]
struct Hints {
    aliases: HashMap<String, String>,
    // Tuple key -> binding name -> count.
    holders: HashMap<String, HashMap<String, usize>>,
    // Tuple key -> the patterns that unpack it, as per-position names (None for `_`), in source order.
    fields: HashMap<String, Vec<Vec<Option<String>>>>,
}

impl Hints {
    fn collect(cx: &Cx, root: &SyntaxNode, nodes: &[SyntaxNode]) -> Hints {
        let mut h = Hints::default();
        for a in root.descendants().filter_map(ast::TypeAlias::cast) {
            if let (Some(n), Some(t)) = (a.name(), a.ty())
                && let Ty::Tuple(ts) = cx.type_ref(&t)
            {
                let ts: Vec<Ty> = ts.iter().map(|t| t.strip_refs().clone()).collect();
                h.aliases.insert(key(&ts), n.text().to_string());
            }
        }
        for n in nodes {
            // Macro expansions name their own temporaries (`assert_eq!`'s `left_val`).
            if !ast::SourceFile::can_cast(n.ancestors().last().map_or(n.kind(), |r| r.kind())) {
                continue;
            }
            if let Some(tp) = ast::TuplePat::cast(n.clone()) {
                let Ty::Tuple(ts) = cx.pat_ty(&ast::Pat::TuplePat(tp.clone())).strip_refs().clone() else {
                    continue;
                };
                let names: Vec<Option<String>> = tp.fields().map(ident_name).collect();
                if names.len() == ts.len() {
                    let ts: Vec<Ty> = ts.iter().map(|t| t.strip_refs().clone()).collect();
                    h.fields.entry(key(&ts)).or_default().push(names);
                }
                continue;
            }
            if let Some(ip) = ast::IdentPat::cast(n.clone()) {
                let Some(name) = ip.name().map(|n| n.text().to_string()) else { continue };
                let ty = cx.pat_ty(&ast::Pat::IdentPat(ip));
                h.holder(ty.strip_refs(), &name);
                continue;
            }
            if let Some(f) = ast::RecordField::cast(n.clone())
                && let (Some(name), Some(t)) = (f.name(), f.ty())
            {
                h.holder(&cx.type_ref(&t), name.text().as_ref());
            }
        }
        h
    }

    fn holder(&mut self, ty: &Ty, name: &str) {
        let (ts, name) = match ty {
            Ty::Tuple(ts) => (ts, name.to_string()),
            // `edges: Vec<(..)>` holds edges: one is an `Edge`.
            Ty::Vec(x) | Ty::Set { t: x, .. } | Ty::Deque(x) | Ty::Heap(x) => match x.strip_refs() {
                Ty::Tuple(ts) if name.len() > 3 && name.ends_with('s') && !name.ends_with("ss") => {
                    (ts, name[..name.len() - 1].to_string())
                }
                _ => return,
            },
            _ => return,
        };
        let ts: Vec<Ty> = ts.iter().map(|t| t.strip_refs().clone()).collect();
        if name.len() < 2 || VAGUE.contains(&name.as_str()) || name.starts_with('_') {
            return;
        }
        *self.holders.entry(key(&ts)).or_default().entry(name).or_default() += 1;
    }

    fn record_name(&self, k: &str) -> Option<String> {
        if let Some(a) = self.aliases.get(k) {
            return Some(a.clone());
        }
        let best = most_common(self.holders.get(k)?)?;
        Some(best.split('_').filter(|s| !s.is_empty()).map(capitalize).collect())
    }

    fn field_names(&self, k: &str, arity: usize) -> Vec<String> {
        let fallback = || -> Vec<String> {
            match arity {
                2 => vec!["first".into(), "second".into()],
                3 => vec!["first".into(), "second".into(), "third".into()],
                _ => (0..arity).map(|i| format!("_{i}")).collect(),
            }
        };
        let usable = |names: &[String]| {
            names.iter().collect::<HashSet<_>>().len() == arity
                && !names.iter().any(|n| RESERVED_FIELDS.contains(&n.as_str()))
        };
        let Some(pats) = self.fields.get(k) else { return fallback() };
        // One pattern's names as a set (mixing `(dr, dc)` with `(r, c)` reads as neither).
        let mut complete: Vec<(Vec<String>, usize)> = Vec::new();
        for p in pats {
            let Some(names) = p.iter().cloned().collect::<Option<Vec<String>>>() else { continue };
            let names: Vec<String> = names.iter().map(|n| super::escape_keyword(n)).collect();
            if !usable(&names) {
                continue;
            }
            match complete.iter_mut().find(|(n, _)| *n == names) {
                Some((_, c)) => *c += 1,
                None => complete.push((names, 1)),
            }
        }
        // Most used wins; ties go to the first in the source (`max_by_key` keeps the last max).
        if let Some((names, _)) = complete.iter().rev().max_by_key(|(_, c)| *c) {
            return names.clone();
        }
        // Only partial patterns (`|&(_, _, w)|`): fill each position from them.
        let picked: Option<Vec<String>> = (0..arity)
            .map(|i| {
                let mut counts: HashMap<String, usize> = HashMap::new();
                for p in pats {
                    if let Some(n) = &p[i] {
                        *counts.entry(n.clone()).or_default() += 1;
                    }
                }
                most_common(&counts).map(|n| super::escape_keyword(&n))
            })
            .collect();
        match picked {
            Some(names) if usable(&names) => names,
            _ => fallback(),
        }
    }
}

// A plain binding's name, through `&x` / `mut x`.
fn ident_name(p: ast::Pat) -> Option<String> {
    match p {
        ast::Pat::RefPat(r) => ident_name(r.pat()?),
        ast::Pat::IdentPat(ip) if ip.pat().is_none() => {
            let n = ip.name()?.text().to_string();
            // `None` / `MAX` in a pattern parse as idents but are constants.
            n.starts_with(|c: char| c.is_lowercase()).then_some(n)
        }
        _ => None,
    }
}

// The most frequent name; ties go to the alphabetically first, so output is stable.
fn most_common(counts: &HashMap<String, usize>) -> Option<String> {
    counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(n, _)| n.clone())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
