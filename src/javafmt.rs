// Java post-pass (DESIGN §22.6): re-lays out printed Java with tree-sitter for simple mode.

use std::collections::{BTreeSet, HashSet};

use tree_sitter::{Node, Parser, Tree};

pub fn parse(src: &str) -> Option<Tree> {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_java::LANGUAGE.into()).ok()?;
    p.parse(src, None)
}

// Line and column (1-based) of the first syntax error, for diagnostics.
pub fn first_error(src: &str) -> Option<(usize, usize)> {
    let tree = parse(&mask_qualified_calls(src)?)?;
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.is_error() || n.is_missing() {
            let p = n.start_position();
            return Some((p.row + 1, p.column + 1));
        }
        if n.has_error() {
            stack.extend(children(n).into_iter().rev());
        }
    }
    None
}

// Normal layout: one statement or member per line, 4-space indents, blank lines where the Rust
// source had them (`blank(l)`: Rust line `l` is empty; exact-mode Java lines are Rust lines).
// `None` when the input doesn't parse.
pub fn relayout(src: &str, blank: &dyn Fn(usize) -> bool, hints: &Hints) -> Option<String> {
    let masked = mask_qualified_calls(src)?;
    relayout_masked(&masked, blank, hints).map(|s| s.replace(MASK, "."))
}

// What lowering knows about the locals.
#[derive(Default)]
pub struct Hints {
    // Locals only macro expansions declare, which may be inlined.
    pub macro_locals: HashSet<String>,
    // User locals renamed for a clash (Java `s3` -> Rust `s`), restored once the clash is gone.
    pub renamed_locals: std::collections::HashMap<String, String>,
}

// tree-sitter-java 0.23 can't parse qualified record patterns (`case Token.Num(var n)`), so
// `Upper.Upper(` is joined into one identifier while laying out. `None` if the mask is in use.
const MASK: &str = "$$";

fn mask_qualified_calls(src: &str) -> Option<String> {
    if src.contains(MASK) {
        return None;
    }
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut last = 0;
    for i in 1..b.len().saturating_sub(1) {
        if b[i] != b'.' || !b[i + 1].is_ascii_uppercase() {
            continue;
        }
        // The segment before the dot is a capitalized identifier...
        let start = src[..i]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
            .map_or(0, |p| p + 1);
        if !b[start].is_ascii_uppercase() || start == i {
            continue;
        }
        // ...and the one after it is a capitalized identifier followed by `(`.
        let end = src[i + 1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(b.len(), |p| i + 1 + p);
        if b.get(end) != Some(&b'(') {
            continue;
        }
        out.push_str(&src[last..i]);
        out.push_str(MASK);
        last = i + 1;
    }
    out.push_str(&src[last..]);
    Some(out)
}

fn relayout_masked(src: &str, blank: &dyn Fn(usize) -> bool, hints: &Hints) -> Option<String> {
    let tree = parse(src)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    let assigned = assigned_names(root, src);
    let uses = ident_uses(root, src);
    let decl_types = declared_types(root, src);
    let (mut renames, mut dropped) = copy_renames(root, src, &assigned);
    let (inlined, gone) = inlines(root, src, &hints.macro_locals, &decl_types, &dropped);
    dropped.extend(gone);
    restore_names(root, src, &hints.renamed_locals, &dropped, &inlined, &mut renames);
    let mut f = Fmt {
        src,
        blank,
        inlined,
        decl_types,
        enum_values_read: reads_value_field(root, src),
        moved: HashSet::new(),
        assigned,
        ident_uses: uses,
        throwing: io_throwing(root, src),
        renames,
        dropped,
    };
    let mut header = f.header_comments(root);
    f.moved = header.iter().map(|c| c.start_byte()).collect();
    let mut out = String::new();
    let mut prev: Option<Node> = None;
    for c in children(root) {
        if let Some(p) = prev {
            let both_imports = is_header(p) && is_header(c);
            out.push('\n');
            if !both_imports && !is_comment(p) {
                out.push('\n');
            }
        }
        // The file's leading comment goes above the class, as a person would put it.
        if c.kind() == "class_declaration" && !header.is_empty() {
            for h in &header {
                out.push_str(f.text(*h));
                out.push('\n');
            }
            header.clear();
        }
        out.push_str(&f.render(c, 0));
        prev = Some(c);
    }
    out.push('\n');
    Some(out)
}

fn is_header(n: Node) -> bool {
    matches!(n.kind(), "import_declaration" | "package_declaration")
}

fn is_comment(n: Node) -> bool {
    matches!(n.kind(), "line_comment" | "block_comment")
}

fn children(n: Node) -> Vec<Node> {
    let mut cur = n.walk();
    n.children(&mut cur).collect()
}

const BODIES: [&str; 6] = [
    "block",
    "class_body",
    "interface_body",
    "constructor_body",
    "switch_block",
    "annotation_type_body",
];

const ATOMS: [&str; 5] = [
    "string_literal",
    "character_literal",
    "text_block",
    "line_comment",
    "block_comment",
];

struct Fmt<'a> {
    src: &'a str,
    blank: &'a dyn Fn(usize) -> bool,
    // Local and parameter names declared with one type throughout the file -> that type.
    decl_types: std::collections::HashMap<String, String>,
    // Some code reads `x.value`, so enums keep their discriminant field.
    enum_values_read: bool,
    // Start bytes of comments printed elsewhere (the file header, above the class).
    moved: HashSet<usize>,
    // Names assigned somewhere after their declaration (`x = ...`).
    assigned: HashSet<String>,
    // How often each identifier appears in the file (declarations included).
    ident_uses: std::collections::HashMap<String, usize>,
    // Methods that call `readLine()`, directly or through each other: `throws IOException`.
    throwing: HashSet<String>,
    // Identifier start byte -> new name, and the copies those renames make redundant.
    renames: std::collections::HashMap<usize, String>,
    dropped: HashSet<usize>,
    // Use start byte -> the initializer of the macro local it inlines.
    inlined: std::collections::HashMap<usize, Node<'a>>,
}

// Names of methods whose body calls `readLine()` or another such method, outside lambdas.
fn io_throwing(root: Node, src: &str) -> HashSet<String> {
    let mut calls: Vec<(String, String)> = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "method_invocation"
            && let Some(name) = n.child_by_field_name("name")
            && !n.ancestors_named().any(|a| a.kind() == "lambda_expression")
            && let Some(m) = n
                .ancestors_named()
                .find(|a| a.kind() == "method_declaration")
                .and_then(|m| m.child_by_field_name("name"))
        {
            let stdin_read = &src[name.byte_range()] == "read"
                && n.child_by_field_name("object").is_some_and(|o| &src[o.byte_range()] == "STDIN");
            let callee = if stdin_read { "readLine" } else { &src[name.byte_range()] };
            calls.push((src[m.byte_range()].to_string(), callee.to_string()));
        }
        stack.extend(children(n));
    }
    let mut out: HashSet<String> = calls
        .iter()
        .filter(|(_, callee)| callee == "readLine")
        .map(|(m, _)| m.clone())
        .collect();
    loop {
        let more: Vec<String> = calls
            .iter()
            .filter(|(m, callee)| out.contains(callee) && !out.contains(m))
            .map(|(m, _)| m.clone())
            .collect();
        if more.is_empty() {
            return out;
        }
        out.extend(more);
    }
}

// `x = e;` -> (x, e).
fn simple_assign(n: Node) -> Option<(Node, Node)> {
    let a = n.named_child(0).filter(|a| {
        n.kind() == "expression_statement" && a.kind() == "assignment_expression"
    })?;
    let op = a.child_by_field_name("operator")?;
    let left = a.child_by_field_name("left")?;
    (op.kind() == "=" && left.kind() == "identifier").then_some((left, a.child_by_field_name("right")?))
}

fn is_literal(n: Node) -> bool {
    matches!(
        n.kind(),
        "string_literal" | "decimal_integer_literal" | "decimal_floating_point_literal"
            | "null_literal" | "true" | "false" | "character_literal"
    )
}

fn mentions(n: Node, src: &str, name: &str) -> bool {
    let mut stack = vec![n];
    while let Some(n) = stack.pop() {
        if n.kind() == "identifier" && &src[n.byte_range()] == name {
            return true;
        }
        stack.extend(children(n));
    }
    false
}

fn idents_named<'t>(n: Node<'t>, src: &str, name: &str) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack = vec![n];
    while let Some(n) = stack.pop() {
        if n.kind() == "identifier" && &src[n.byte_range()] == name {
            out.push(n);
        }
        stack.extend(children(n));
    }
    out
}

fn in_lambda(n: Node, stop: Node) -> bool {
    let mut cur = n.parent();
    while let Some(p) = cur {
        if p == stop {
            return false;
        }
        if matches!(p.kind(), "lambda_expression" | "class_body") {
            return true;
        }
        cur = p.parent();
    }
    false
}

// `T a ...; ..; T b = a;` with `a` used nowhere else: `a` is renamed `b` and the copy goes.
// Returns identifier start byte -> new name, and the start bytes of the dropped copies.
fn copy_renames(
    root: Node,
    src: &str,
    assigned: &HashSet<String>,
) -> (std::collections::HashMap<usize, String>, HashSet<usize>) {
    let mut renames = std::collections::HashMap::new();
    let mut dropped = HashSet::new();
    let mut stack = vec![root];
    while let Some(block) = stack.pop() {
        stack.extend(children(block));
        if block.kind() != "block" {
            continue;
        }
        let stmts: Vec<Node> = children(block).into_iter().filter(|c| c.is_named()).collect();
        for (q, &copy) in stmts.iter().enumerate() {
            let Some(d) = single_declarator(copy) else { continue };
            let (Some(b), Some(a)) = (
                d.child_by_field_name("name"),
                d.child_by_field_name("value").filter(|v| v.kind() == "identifier"),
            ) else {
                continue;
            };
            let (a, b) = (&src[a.byte_range()], &src[b.byte_range()]);
            let Some(p) = stmts[..q].iter().position(|s| {
                single_declarator(*s)
                    .and_then(|d| d.child_by_field_name("name"))
                    .is_some_and(|n| &src[n.byte_range()] == a)
            }) else {
                continue;
            };
            let ty = |s: Node| s.child_by_field_name("type").map(|t| &src[t.byte_range()]);
            let same_type = ty(copy) == Some("var") || ty(copy) == ty(stmts[p]);
            let uses: Vec<Node> = stmts[p..q].iter().flat_map(|s| idents_named(*s, src, a)).collect();
            // Every mention is a plain local: none is a field name or a method name.
            let plain = uses.iter().all(|u| {
                u.parent().is_none_or(|par| {
                    par.child_by_field_name("field") != Some(*u)
                        && !(par.kind() == "method_invocation" && par.child_by_field_name("name") == Some(*u))
                })
            });
            let written = uses.iter().any(|u| {
                u.parent().is_some_and(|par| {
                    par.kind() == "update_expression"
                        || par.kind() == "assignment_expression" && par.child_by_field_name("left") == Some(*u)
                })
            });
            // A lambda captures only effectively final locals, which renaming must not change.
            let captured_a = uses.iter().any(|u| in_lambda(*u, block));
            let captured_b = stmts[q + 1..]
                .iter()
                .flat_map(|s| idents_named(*s, src, b))
                .any(|u| in_lambda(u, block));
            if !same_type
                || !plain
                || stmts[p..].iter().map(|s| idents_named(*s, src, a).len()).sum::<usize>() != uses.len() + 1
                || stmts[p..q].iter().any(|s| mentions(*s, src, b))
                || written && captured_b
                || captured_a && assigned.contains(b)
                || uses.iter().any(|u| renames.contains_key(&u.start_byte()))
            {
                continue;
            }
            for u in uses {
                renames.insert(u.start_byte(), b.to_string());
            }
            dropped.insert(copy.start_byte());
        }
    }
    (renames, dropped)
}

// `String s = f(); var n = Integer.parseInt(s);` for a macro's local `s`: `s` is used once, by
// the next statement, before anything there with an effect, so the call moves into its place.
// Returns use start byte -> the initializer, and the start bytes of the dropped declarations.
fn inlines<'t>(
    root: Node<'t>,
    src: &str,
    inlinable: &HashSet<String>,
    locals: &std::collections::HashMap<String, String>,
    copies: &HashSet<usize>,
) -> (std::collections::HashMap<usize, Node<'t>>, HashSet<usize>) {
    let mut uses = std::collections::HashMap::new();
    let mut dropped = HashSet::new();
    let mut stack = vec![root];
    while let Some(block) = stack.pop() {
        stack.extend(children(block));
        if block.kind() != "block" {
            continue;
        }
        let stmts: Vec<Node> = children(block).into_iter().filter(|c| c.is_named()).collect();
        for (at, pair) in stmts.windows(2).enumerate() {
            let (decl, next) = (pair[0], pair[1]);
            let Some(d) = single_declarator(decl) else { continue };
            let (Some(name), Some(init)) = (d.child_by_field_name("name"), d.child_by_field_name("value"))
            else {
                continue;
            };
            let name = &src[name.byte_range()];
            let ty = decl.child_by_field_name("type").map(|t| &src[t.byte_range()]);
            let primary = matches!(init.kind(), "method_invocation" | "object_creation_expression");
            let found = idents_named(next, src, name);
            let later: usize = stmts[at + 2..].iter().map(|s| idents_named(*s, src, name).len()).sum();
            let ([u], 0) = (found.as_slice(), later) else { continue };
            if !inlinable.contains(name)
                || !matches!(ty, Some("String" | "var"))
                || !primary
                || copies.contains(&next.start_byte())
                || !straight_path(*u, next)
                || !pure_prefix(next, *u, src, locals)
            {
                continue;
            }
            uses.insert(u.start_byte(), init);
            dropped.insert(decl.start_byte());
        }
    }
    (uses, dropped)
}

// `u` runs exactly once when `stmt` runs: not in a lambda, a loop, a `?:` branch or the right of
// `&&`/`||`, and `stmt` is a plain declaration, expression, return, throw or `if` condition.
fn straight_path(u: Node, stmt: Node) -> bool {
    if !matches!(
        stmt.kind(),
        "local_variable_declaration" | "expression_statement" | "return_statement"
            | "throw_statement" | "if_statement"
    ) {
        return false;
    }
    let mut cur = u;
    while let Some(p) = cur.parent() {
        if p == stmt {
            return stmt.kind() != "if_statement" || p.child_by_field_name("condition") == Some(cur);
        }
        let ok = match p.kind() {
            "ternary_expression" => p.child_by_field_name("condition") == Some(cur),
            "binary_expression" => {
                let op = p.child_by_field_name("operator").map(|o| o.kind());
                !matches!(op, Some("&&" | "||")) || p.child_by_field_name("left") == Some(cur)
            }
            "lambda_expression" | "switch_expression" | "class_body" => false,
            _ => true,
        };
        if !ok {
            return false;
        }
        cur = p;
    }
    false
}

// Nothing in `stmt` evaluated before `u` has an effect or reads state a call could change: only
// literals, locals, class names and their constants come first.
fn pure_prefix(
    stmt: Node,
    u: Node,
    src: &str,
    locals: &std::collections::HashMap<String, String>,
) -> bool {
    let upper = |n: Node| src[n.byte_range()].starts_with(|c: char| c.is_ascii_uppercase());
    let mut stack = vec![stmt];
    while let Some(n) = stack.pop() {
        if n.start_byte() >= u.start_byte() {
            continue;
        }
        stack.extend(children(n));
        if n.end_byte() > u.start_byte() {
            continue;
        }
        let parent = n.parent();
        let field_of = |f: &str| parent.is_some_and(|p| p.child_by_field_name(f) == Some(n));
        let ok = match n.kind() {
            "method_invocation" | "object_creation_expression" | "assignment_expression"
            | "update_expression" | "array_creation_expression" | "lambda_expression" | "this" => false,
            "field_access" => n.child_by_field_name("object").is_some_and(|o| o.kind() == "identifier" && upper(o)),
            "identifier" => {
                field_of("name") || field_of("field") || upper(n) || locals.contains_key(&src[n.byte_range()])
            }
            _ => true,
        };
        if !ok {
            return false;
        }
    }
    true
}

// Per method: a user local renamed for a clash (`s3`) takes its Rust name (`s`) back when nothing
// printed there still uses that name.
fn restore_names(
    root: Node,
    src: &str,
    renamed: &std::collections::HashMap<String, String>,
    dropped: &HashSet<usize>,
    inlined: &std::collections::HashMap<usize, Node>,
    renames: &mut std::collections::HashMap<usize, String>,
) {
    if renamed.is_empty() {
        return;
    }
    let mut pairs: Vec<(&String, &String)> = renamed.iter().collect();
    pairs.sort();
    let mut stack = vec![root];
    while let Some(m) = stack.pop() {
        if !matches!(m.kind(), "method_declaration" | "constructor_declaration") {
            stack.extend(children(m));
            continue;
        }
        let within = |n: Node, outer: Node| outer.start_byte() <= n.start_byte() && n.end_byte() <= outer.end_byte();
        let inits: Vec<Node> = inlined.values().copied().filter(|i| within(*i, m)).collect();
        // Identifiers that still print: outside dropped statements, or in an inlined initializer.
        let mut printed = Vec::new();
        let mut walk = vec![m];
        while let Some(n) = walk.pop() {
            walk.extend(children(n));
            if n.kind() != "identifier" || inlined.contains_key(&n.start_byte()) {
                continue;
            }
            let mut up = n.parent();
            let mut in_dropped = false;
            while let Some(p) = up.filter(|p| *p != m) {
                let stmt = p.kind() == "local_variable_declaration" || p.kind().ends_with("statement");
                in_dropped |= stmt && dropped.contains(&p.start_byte());
                up = p.parent();
            }
            if !in_dropped || inits.iter().any(|i| within(n, *i)) {
                printed.push(n);
            }
        }
        let member = |n: &Node| {
            n.parent().is_some_and(|p| {
                p.child_by_field_name("field") == Some(*n)
                    || p.kind() == "method_invocation" && p.child_by_field_name("name") == Some(*n)
            })
        };
        for (java, rust) in &pairs {
            let name = |n: &Node| {
                renames.get(&n.start_byte()).cloned().unwrap_or_else(|| src[n.byte_range()].to_string())
            };
            let free = !printed.iter().any(|n| name(n) == **rust);
            let hits: Vec<usize> = printed
                .iter()
                .filter(|n| name(n) == **java && !member(n))
                .map(|n| n.start_byte())
                .collect();
            if free && !hits.is_empty() {
                for h in hits {
                    renames.insert(h, rust.to_string());
                }
            }
        }
    }
}

// The one declarator of `T x = e;`.
fn single_declarator(n: Node) -> Option<Node> {
    if n.kind() != "local_variable_declaration" || n.named_child_count() != 2 {
        return None;
    }
    n.child_by_field_name("declarator")
}

fn ident_uses(root: Node, src: &str) -> std::collections::HashMap<String, usize> {
    let mut out = std::collections::HashMap::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "identifier" {
            *out.entry(src[n.byte_range()].to_string()).or_insert(0) += 1;
        }
        stack.extend(children(n));
    }
    out
}

fn assigned_names(root: Node, src: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "assignment_expression"
            && let Some(l) = n.child_by_field_name("left")
            && l.kind() == "identifier"
        {
            out.insert(src[l.byte_range()].to_string());
        }
        stack.extend(children(n));
    }
    out
}

fn reads_value_field(root: Node, src: &str) -> bool {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        // `this.value = value` in the enum's constructor is a write.
        let written = n.parent().is_some_and(|p| {
            p.kind() == "assignment_expression" && p.child_by_field_name("left") == Some(n)
        });
        if n.kind() == "field_access"
            && !written
            && n.child_by_field_name("field").is_some_and(|f| &src[f.byte_range()] == "value")
        {
            return true;
        }
        stack.extend(children(n));
    }
    false
}

// An expression Java types as String: a literal, a concatenation with one, `String.valueOf(..)`.
fn is_string_expr(n: Node, src: &str) -> bool {
    match n.kind() {
        "string_literal" => true,
        "binary_expression" => {
            n.child_by_field_name("operator").is_some_and(|o| &src[o.byte_range()] == "+")
                && [n.child_by_field_name("left"), n.child_by_field_name("right")]
                    .into_iter()
                    .flatten()
                    .any(|c| is_string_expr(c, src))
        }
        "parenthesized_expression" => n.named_child(0).is_some_and(|c| is_string_expr(c, src)),
        "method_invocation" => is_value_of(n, src),
        _ => false,
    }
}

// Whether `n`'s type comes from a declared target: an explicitly typed initializer, an assignment
// or a method's `return`, possibly through parentheses and `?:` branches.
fn typed_slot(n: Node, src: &str) -> bool {
    let mut cur = n;
    while let Some(p) = cur.parent() {
        match p.kind() {
            "parenthesized_expression" => {}
            "ternary_expression" if p.child_by_field_name("condition") != Some(cur) => {}
            "variable_declarator" => {
                let typed = p.parent().and_then(|d| d.child_by_field_name("type"));
                return typed.is_some_and(|t| &src[t.byte_range()] != "var");
            }
            "assignment_expression" => return p.child_by_field_name("right") == Some(cur),
            "return_statement" => {
                // A lambda's return type is inferred, so it gives no target.
                let mut up = p.parent();
                while let Some(u) = up {
                    match u.kind() {
                        "lambda_expression" => return false,
                        "method_declaration" => return true,
                        _ => up = u.parent(),
                    }
                }
                return false;
            }
            _ => return false,
        }
        cur = p;
    }
    false
}

// `String.valueOf(x)` with one argument.
fn is_value_of(n: Node, src: &str) -> bool {
    n.kind() == "method_invocation"
        && n.child_by_field_name("object").is_some_and(|o| &src[o.byte_range()] == "String")
        && n.child_by_field_name("name").is_some_and(|o| &src[o.byte_range()] == "valueOf")
        && n.child_by_field_name("arguments").is_some_and(|a| a.named_child_count() == 1)
}

// Names whose every declaration (local, parameter, for-each variable) has the same type text.
fn declared_types(root: Node, src: &str) -> std::collections::HashMap<String, String> {
    let mut seen: std::collections::HashMap<String, Option<String>> = Default::default();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let decl = match n.kind() {
            "local_variable_declaration" => n
                .child_by_field_name("type")
                .zip(n.child_by_field_name("declarator").and_then(|d| d.child_by_field_name("name"))),
            "formal_parameter" | "enhanced_for_statement" | "catch_formal_parameter" => {
                n.child_by_field_name("type").zip(n.child_by_field_name("name"))
            }
            _ => None,
        };
        if let Some((t, name)) = decl {
            let t = src[t.byte_range()].to_string();
            let entry = seen.entry(src[name.byte_range()].to_string()).or_insert(Some(t.clone()));
            if entry.as_deref() != Some(t.as_str()) {
                *entry = None;
            }
        }
        stack.extend(children(n));
    }
    seen.into_iter().filter_map(|(k, v)| Some((k, v?))).collect()
}

fn indent(n: usize) -> String {
    "    ".repeat(n)
}

impl Fmt<'_> {
    fn text(&self, n: Node) -> &str {
        &self.src[n.byte_range()]
    }

    // Comments opening the top class body and set off from the first member by a blank Rust
    // line: the file's own header comment.
    fn header_comments<'t>(&self, root: Node<'t>) -> Vec<Node<'t>> {
        let Some(body) = children(root)
            .into_iter()
            .find(|c| c.kind() == "class_declaration")
            .and_then(|c| c.child_by_field_name("body"))
        else {
            return Vec::new();
        };
        let items: Vec<Node> = children(body).into_iter().filter(|c| c.is_named()).collect();
        let lead: Vec<Node> = items.iter().copied().take_while(|c| is_comment(*c)).collect();
        match (lead.last(), items.get(lead.len())) {
            (Some(last), Some(next)) if self.blank_between(*last, *next, false) => lead,
            (Some(_), None) => lead,
            _ => Vec::new(),
        }
    }

    fn render(&self, n: Node, ind: usize) -> String {
        if BODIES.contains(&n.kind()) {
            return self.body(n, ind);
        }
        if n.kind() == "enum_body" {
            return self.enum_body(n, ind);
        }
        if n.kind() == "switch_block_statement_group" {
            return self.case_group(n, ind);
        }
        if n.kind() == "identifier" {
            if let Some(name) = self.renames.get(&n.start_byte()) {
                return name.clone();
            }
            if let Some(init) = self.inlined.get(&n.start_byte()) {
                return self.render(*init, ind);
            }
        }
        if n.child_count() == 0 || ATOMS.contains(&n.kind()) {
            return self.text(n).trim_end().to_string();
        }
        if let Some(text) = self.var_list(n, ind).or_else(|| self.idiom(n, ind)) {
            return text;
        }
        let plain = self.plain(n);
        if plain != n {
            return self.render(plain, ind);
        }
        let mut out = String::new();
        let mut prev: Option<Node> = None;
        // `readLine()` throws a checked IOException; the method declares it.
        let throws = n.kind() == "method_declaration"
            && n.child_by_field_name("name")
                .is_some_and(|m| self.throwing.contains(self.text(m)))
            && !children(n).iter().any(|c| c.kind() == "throws");
        for c in children(n) {
            if throws && c.kind() == "block" {
                out.push_str(" throws IOException");
            }
            if let Some(p) = prev {
                out.push_str(&self.gap(p, c, ind));
            }
            // `if (c) x; else y;` and brace-less loop bodies get braces and their own lines.
            let branch = ["consequence", "alternative", "body"]
                .iter()
                .any(|f| n.child_by_field_name(f) == Some(c));
            let else_if = n.kind() == "if_statement"
                && c.kind() == "if_statement"
                && n.child_by_field_name("alternative") == Some(c);
            if branch
                && matches!(
                    n.kind(),
                    "if_statement" | "while_statement" | "for_statement"
                        | "enhanced_for_statement" | "do_statement"
                )
                && c.kind() != "block"
                && !else_if
            {
                out.push_str(&format!(
                    "{{\n{}{}\n{}}}",
                    indent(ind + 1),
                    self.render(c, ind + 1),
                    indent(ind)
                ));
            } else {
                out.push_str(&self.render(c, ind));
            }
            prev = Some(c);
        }
        out
    }

    // `List<T> v = new ArrayList<>(..);` -> `var v = new ArrayList<T>(..);` when `v` is never
    // reassigned (a `Vec` stays a mutable `ArrayList`, typed as one).
    fn var_list(&self, n: Node, ind: usize) -> Option<String> {
        if n.kind() != "local_variable_declaration" || n.named_child_count() != 2 {
            return None;
        }
        let ty = self.text(n.child_by_field_name("type")?);
        let targs = ty.strip_prefix("List")?;
        let decl = n.child_by_field_name("declarator")?;
        let name = self.render(decl.child_by_field_name("name")?, ind);
        let value = decl.child_by_field_name("value")?;
        if !targs.starts_with('<')
            || self.assigned.contains(self.text(decl.child_by_field_name("name")?))
            || self.assigned.contains(&name)
            || value.kind() != "object_creation_expression"
            || value.child_by_field_name("type").map(|t| self.text(t)) != Some("ArrayList<>")
            || children(value).iter().any(|c| c.kind() == "class_body")
        {
            return None;
        }
        let args = self.render(value.child_by_field_name("arguments")?, ind);
        Some(format!("var {name} = new ArrayList{targs}{args};"))
    }

    // The shorter JDK spelling of a generated expression, when one means the same thing.
    fn idiom(&self, n: Node, ind: usize) -> Option<String> {
        match n.kind() {
            "method_invocation" => {}
            "lambda_expression" => return self.method_ref(n),
            _ => return None,
        }
        let name = self.text(n.child_by_field_name("name")?);
        let object = n.child_by_field_name("object");
        let args: Vec<Node> = n
            .child_by_field_name("arguments")
            .map(|a| children(a).into_iter().filter(|c| c.is_named() && !is_comment(*c)).collect())
            .unwrap_or_default();
        // `Arrays.stream(a)` of an object array, as its one argument.
        let stream_of_array = |o: Node| -> Option<String> {
            let inner: Vec<Node> = o
                .child_by_field_name("arguments")
                .map(|a| children(a).into_iter().filter(|c| c.is_named()).collect())?;
            (self.text(o.child_by_field_name("object")?) == "Arrays"
                && self.text(o.child_by_field_name("name")?) == "stream"
                && inner.len() == 1)
                .then(|| self.render(inner[0], ind))
        };
        match (name, object, args.as_slice()) {
            // A literal char made a String is a String literal.
            ("valueOf", Some(o), [c]) if self.text(o) == "String" && c.kind() == "character_literal" => {
                let inner = &self.text(*c)[1..self.text(*c).len() - 1];
                let inner = match inner {
                    "\\'" => "'",
                    "\"" => "\\\"",
                    other => other,
                };
                Some(format!("\"{inner}\""))
            }
            // `toList()` is never mutated, so the fixed-size `asList` view serves.
            ("toList", Some(o), []) => Some(format!("Arrays.asList({})", stream_of_array(o)?)),
            ("collect", Some(o), [c]) if self.text(*c) == "Collectors.toCollection(ArrayList::new)" => {
                if let Some(a) = stream_of_array(o) {
                    return Some(format!("new ArrayList<>(Arrays.asList({a}))"));
                }
                let src = o.child_by_field_name("object")?;
                let empty = o.child_by_field_name("arguments")?.named_child_count() == 0;
                let collection = self.decl_types.get(self.text(src)).is_some_and(|t| {
                    ["List<", "ArrayList<", "Set<", "HashSet<", "TreeSet<", "Deque<", "ArrayDeque<"]
                        .iter()
                        .any(|p| t.starts_with(p))
                });
                (src.kind() == "identifier"
                    && self.text(o.child_by_field_name("name")?) == "stream"
                    && empty
                    && collection)
                    .then(|| format!("new ArrayList<>({})", self.text(src)))
            }
            ("empty", Some(o), [])
                if self.text(o) == "Optional"
                    && n.child_by_field_name("type_arguments").is_some()
                    && typed_slot(n, self.src) =>
            {
                Some("Optional.empty()".into())
            }
            _ => None,
        }
    }

    // `s -> Integer.parseInt(s)` -> `Integer::parseInt`.
    fn method_ref(&self, n: Node) -> Option<String> {
        let p = n.child_by_field_name("parameters").filter(|p| p.kind() == "identifier")?;
        let body = n.child_by_field_name("body").filter(|b| b.kind() == "method_invocation")?;
        let class = self.text(body.child_by_field_name("object")?);
        let method = self.text(body.child_by_field_name("name")?);
        let args = body.child_by_field_name("arguments")?;
        let parsers = [
            ("Integer", "parseInt"),
            ("Long", "parseLong"),
            ("Double", "parseDouble"),
            ("Integer", "parseUnsignedInt"),
            ("Long", "parseUnsignedLong"),
            ("Character", "isDigit"),
            ("Character", "isLetter"),
            ("Character", "isLetterOrDigit"),
            ("Character", "isWhitespace"),
            ("Character", "isUpperCase"),
            ("Character", "isLowerCase"),
        ];
        (parsers.contains(&(class, method))
            && args.named_child_count() == 1
            && args.named_child(0).is_some_and(|a| self.text(a) == self.text(p)))
        .then(|| format!("{class}::{method}"))
    }

    // `n` without redundant wrapping: a cast of a local to its declared type, or parentheses
    // around a primary expression or in a slot that takes any expression.
    fn plain<'t>(&self, n: Node<'t>) -> Node<'t> {
        match n.kind() {
            // `"a" + String.valueOf(x)`: the concatenation already converts `x`.
            "method_invocation" if is_value_of(n, self.src) => {
                let Some(arg) = n.child_by_field_name("arguments").and_then(|a| a.named_child(0))
                else {
                    return n;
                };
                let Some(parent) = n.parent().filter(|p| {
                    p.kind() == "binary_expression"
                        && p.child_by_field_name("operator")
                            .is_some_and(|o| self.text(o) == "+")
                }) else {
                    return n;
                };
                let right = parent.child_by_field_name("right") == Some(n);
                let other = if right {
                    parent.child_by_field_name("left")
                } else {
                    // `String.valueOf(a) + String.valueOf(b)` must keep one of the two.
                    parent
                        .child_by_field_name("right")
                        .filter(|o| !is_value_of(*o, self.src))
                };
                let tight = matches!(
                    arg.kind(),
                    "identifier" | "field_access" | "method_invocation" | "array_access"
                        | "parenthesized_expression" | "decimal_integer_literal"
                        | "character_literal" | "object_creation_expression"
                );
                if tight && other.is_some_and(|o| is_string_expr(o, self.src)) {
                    arg
                } else {
                    n
                }
            }
            "cast_expression" => {
                let (Some(t), Some(v)) = (n.child_by_field_name("type"), n.child_by_field_name("value"))
                else {
                    return n;
                };
                let v = self.plain(v);
                let same = v.kind() == "identifier"
                    && self.decl_types.get(self.text(v)).map(String::as_str) == Some(self.text(t));
                if same { v } else { n }
            }
            "parenthesized_expression" => {
                let Some(parent) = n.parent() else {
                    return n;
                };
                // `if (c)`, `while (c)`, `switch (x)`: these parentheses are syntax.
                if matches!(
                    parent.kind(),
                    "if_statement" | "while_statement" | "do_statement" | "switch_expression"
                        | "switch_statement" | "synchronized_statement"
                ) {
                    return n;
                }
                let Some(inner) = n.named_child(0) else {
                    return n;
                };
                if n.named_child_count() != 1 {
                    return n;
                }
                let inner = self.plain(inner);
                let primary = matches!(
                    inner.kind(),
                    "identifier" | "decimal_integer_literal" | "decimal_floating_point_literal"
                        | "string_literal" | "character_literal" | "true" | "false"
                        | "null_literal" | "this" | "method_invocation" | "field_access"
                        | "array_access" | "parenthesized_expression"
                );
                let any_slot = matches!(
                    parent.kind(),
                    "argument_list" | "variable_declarator" | "return_statement"
                        | "parenthesized_expression" | "expression_statement"
                ) || (parent.kind() == "assignment_expression"
                    && parent.child_by_field_name("right") == Some(n));
                if primary || any_slot { inner } else { n }
            }
            _ => n,
        }
    }

    // Whitespace between two siblings inside one statement or expression.
    fn gap(&self, p: Node, c: Node, ind: usize) -> String {
        if p.kind() == "line_comment" {
            return format!("\n{}", indent(ind + 1));
        }
        let gap = &self.src[p.end_byte()..c.start_byte()];
        if gap.is_empty() {
            return String::new();
        }
        if !gap.contains('\n') {
            return " ".into();
        }
        let next = self.src[c.start_byte()..].chars().next().unwrap_or(' ');
        let last = self.src[..p.end_byte()].chars().last().unwrap_or(' ');
        if matches!(next, '.' | ')' | ']' | ',' | ';' | ':') && c.kind() != "?"
            || matches!(last, '(' | '[' | '.')
        {
            String::new()
        } else {
            " ".into()
        }
    }

    fn items(&self, items: &[Node], ind: usize, class: bool) -> String {
        let mut out = String::new();
        let mut prev: Option<Node> = None;
        let mut skip = None;
        for (at, &c) in items.iter().enumerate() {
            if skip == Some(at) || self.dropped.contains(&c.start_byte()) {
                continue;
            }
            // `x = "";` right before `x = e;` is never read.
            if let (Some((l1, r1)), Some((l2, r2))) =
                (simple_assign(c), items.get(at + 1).and_then(|n| simple_assign(*n)))
                && self.text(l1) == self.text(l2)
                && is_literal(r1)
                && !mentions(r2, self.src, self.text(l1))
            {
                continue;
            }
            // `List<T> xs = e; ListIterator<T> it = xs.listIterator();` with `xs` used nowhere
            // else: one line, `ListIterator<T> it = e.listIterator();`. Likewise
            // `String x = ""; x = e;` is `String x = e;`.
            if let Some(&next) = items.get(at + 1)
                && let Some(merged) = self
                    .merge_list_iterator(c, next, ind)
                    .or_else(|| self.merge_initializer(c, next, ind))
            {
                if let Some(p) = prev
                    && self.blank_between(p, c, class)
                {
                    out.push('\n');
                }
                out.push('\n');
                out.push_str(&indent(ind));
                out.push_str(&merged);
                prev = Some(next);
                skip = Some(at + 1);
                continue;
            }
            if let Some(p) = prev {
                if is_comment(c) && p.end_position().row == c.start_position().row {
                    out.push(' ');
                    out.push_str(self.text(c));
                    prev = Some(c);
                    continue;
                }
                if self.blank_between(p, c, class) {
                    out.push('\n');
                }
            }
            out.push('\n');
            out.push_str(&indent(ind));
            out.push_str(&self.render(c, ind));
            prev = Some(c);
        }
        out
    }

    fn merge_initializer(&self, decl: Node, assign: Node, ind: usize) -> Option<String> {
        let d = single_declarator(decl)?;
        let name = self.text(d.child_by_field_name("name")?);
        let (left, right) = simple_assign(assign)?;
        if self.text(left) != name
            || !is_literal(d.child_by_field_name("value")?)
            || mentions(right, self.src, name)
        {
            return None;
        }
        let ty = self.text(decl.child_by_field_name("type")?);
        let name = self.render(d.child_by_field_name("name")?, ind);
        // `var` would take the literal's type; keep the declared one.
        Some(format!("{ty} {name} = {};", self.render(right, ind)))
    }

    fn merge_list_iterator(&self, list: Node, iter: Node, ind: usize) -> Option<String> {
        let (ld, id) = (single_declarator(list)?, single_declarator(iter)?);
        let name = self.text(ld.child_by_field_name("name")?);
        let init = ld.child_by_field_name("value")?;
        let call = id.child_by_field_name("value")?;
        let reads_list = call.kind() == "method_invocation"
            && call.child_by_field_name("object").map(|o| self.text(o)) == Some(name)
            && call.child_by_field_name("name").map(|o| self.text(o)) == Some("listIterator")
            && call
                .child_by_field_name("arguments")
                .is_some_and(|a| a.named_child_count() == 0);
        let chainable = matches!(
            init.kind(),
            "method_invocation" | "object_creation_expression" | "identifier"
        );
        if !reads_list || !chainable || self.ident_uses.get(name).copied() != Some(2) {
            return None;
        }
        Some(format!(
            "{} {} = {}.listIterator();",
            self.text(iter.child_by_field_name("type")?),
            self.render(id.child_by_field_name("name")?, ind),
            self.render(init, ind)
        ))
    }

    fn blank_between(&self, p: Node, c: Node, class: bool) -> bool {
        let from = p.end_position().row + 1;
        let to = c.start_position().row + 1;
        if (from + 1..to).any(|l| (self.blank)(l)) {
            return true;
        }
        // Members other than fields always get a separating line.
        class && !is_comment(p) && !(is_field(p) && is_field(c))
    }

    fn body(&self, n: Node, ind: usize) -> String {
        let mut items: Vec<Node> = children(n)
            .into_iter()
            .filter(|c| c.is_named() || !matches!(c.kind(), "{" | "}" | ";"))
            .filter(|c| !self.moved.contains(&c.start_byte()))
            .collect();
        if items.is_empty() {
            return "{}".into();
        }
        let class = matches!(n.kind(), "class_body" | "interface_body" | "annotation_type_body");
        if n.kind() == "class_body" && n.parent().and_then(|p| p.parent()).is_some_and(|g| g.kind() == "program") {
            // Static fields first, as people write them; relative order (and so init order) is kept.
            items.sort_by_key(|c| !is_field(*c));
        }
        format!("{{{}\n{}}}", self.items(&items, ind + 1, class), indent(ind))
    }

    fn enum_body(&self, n: Node, ind: usize) -> String {
        let all = children(n);
        // The generated discriminant (`Small(0)`, `final int value`, its constructor) goes when
        // nothing reads `.value`.
        let drop_values = !self.enum_values_read;
        let consts: Vec<String> = all
            .iter()
            .filter(|c| c.kind() == "enum_constant")
            .map(|c| match c.child_by_field_name("name") {
                Some(name) if drop_values && c.child_by_field_name("body").is_none() => {
                    self.text(name).to_string()
                }
                _ => self.render(*c, ind + 1),
            })
            .collect();
        let decls: Vec<Node> = all
            .iter()
            .filter(|c| c.kind() == "enum_body_declarations")
            .flat_map(|d| children(*d))
            .filter(|c| c.is_named())
            .filter(|c| {
                let text = self.text(*c);
                !(drop_values
                    && (text == "public final int value;"
                        || c.kind() == "constructor_declaration" && text.contains("(int value)")))
            })
            .collect();
        let mut out = format!("{{\n{}{}", indent(ind + 1), consts.join(", "));
        if !decls.is_empty() {
            out.push_str(";\n");
            out.push_str(&self.items(&decls, ind + 1, true));
        }
        out.push_str(&format!("\n{}}}", indent(ind)));
        out
    }

    // `case X:` followed by statements (old-style switch).
    fn case_group(&self, n: Node, ind: usize) -> String {
        let mut head = String::new();
        let mut stmts = Vec::new();
        for c in children(n) {
            if c.kind() == "switch_label" || c.kind() == ":" {
                head.push_str(&self.render(c, ind));
            } else {
                stmts.push(c);
            }
        }
        head + &self.items(&stmts, ind + 1, false)
    }
}

fn is_field(n: Node) -> bool {
    matches!(
        n.kind(),
        "field_declaration" | "static_initializer" | "constant_declaration"
    )
}

// Generated methods a person wouldn't write unless something calls them.
const GENERATED: [&str; 8] = [
    "debug",
    "clone",
    "partial_cmp",
    "debug_str",
    "debug_float",
    "debug_any",
    "debug_pretty",
    "display_any",
];

// Drops generated methods (and the embedded `Result` type's methods) that nothing calls. Removed
// text is blanked to spaces so line numbers, which `relayout` maps to Rust lines, stay put.
pub fn drop_unused(src: &str) -> String {
    let mut text = src.to_string();
    loop {
        let Some(masked) = mask_qualified_calls(&text) else {
            return text;
        };
        let Some(tree) = parse(&masked) else {
            return text;
        };
        if tree.root_node().has_error() {
            return text;
        }
        let mut calls = Vec::new();
        let mut decls = Vec::new();
        collect_calls(tree.root_node(), &masked, None, &mut calls, &mut decls);
        let called = live_names(&calls);
        let dead: Vec<(usize, usize)> = decls
            .into_iter()
            .filter(|(name, _, _)| !called.contains(name))
            .map(|(_, s, e)| (s, e))
            .collect();
        if dead.is_empty() {
            return text;
        }
        // The ranges are in masked text: blank there, then unmask.
        let mut bytes = masked.into_bytes();
        for (s, e) in dead {
            for b in &mut bytes[s..e] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
        }
        text = String::from_utf8(bytes).unwrap_or_default().replace(MASK, ".");
    }
}

// Calls as (callee, the candidate method containing the call), and the candidate declarations.
fn collect_calls(
    n: Node,
    src: &str,
    inside: Option<&str>,
    calls: &mut Vec<(String, Option<String>)>,
    decls: &mut Vec<(String, usize, usize)>,
) {
    let text = |n: Node| src[n.byte_range()].to_string();
    let mut here = inside.map(str::to_string);
    match n.kind() {
        "method_invocation" => {
            if let Some(name) = n.child_by_field_name("name") {
                calls.push((text(name), here.clone()));
            }
        }
        "method_reference" => {
            if let Some(last) = n.named_child((n.named_child_count() as u32).saturating_sub(1)) {
                calls.push((text(last), here.clone()));
            }
        }
        "method_declaration" => {
            if let Some(name) = n.child_by_field_name("name") {
                let name = text(name);
                let in_result = n.ancestors_named().any(|a| {
                    matches!(a.kind(), "interface_declaration" | "record_declaration")
                        && a.child_by_field_name("name")
                            .is_some_and(|x| matches!(&src[x.byte_range()], "Result" | "Ok" | "Err"))
                });
                if GENERATED.contains(&name.as_str()) || in_result {
                    decls.push((name.clone(), n.start_byte(), n.end_byte()));
                    here = Some(name);
                }
            }
        }
        _ => {}
    }
    for c in children(n) {
        collect_calls(c, src, here.as_deref(), calls, decls);
    }
}

// Names called from live code: calls outside candidates, then calls inside live candidates.
fn live_names(calls: &[(String, Option<String>)]) -> HashSet<String> {
    let mut live: HashSet<String> = calls
        .iter()
        .filter(|(_, inside)| inside.is_none())
        .map(|(name, _)| name.clone())
        .collect();
    loop {
        let more: Vec<String> = calls
            .iter()
            .filter(|(name, inside)| {
                !live.contains(name) && inside.as_ref().is_some_and(|i| live.contains(i))
            })
            .map(|(name, _)| name.clone())
            .collect();
        if more.is_empty() {
            return live;
        }
        live.extend(more);
    }
}

trait Ancestors<'t> {
    fn ancestors_named(&self) -> impl Iterator<Item = Node<'t>>;
}

impl<'t> Ancestors<'t> for Node<'t> {
    fn ancestors_named(&self) -> impl Iterator<Item = Node<'t>> {
        std::iter::successors(self.parent(), |p| p.parent())
    }
}

// Replaces wildcard imports with the used classes, and drops unused single imports.
pub fn explicit_imports(src: &str) -> String {
    // Masking happens after the imports, so their byte offsets are the same in `src`.
    let Some(masked) = mask_qualified_calls(src) else {
        return src.to_string();
    };
    let Some(tree) = parse(&masked) else {
        return src.to_string();
    };
    let root = tree.root_node();
    if root.has_error() {
        return src.to_string();
    }
    let mut declared = HashSet::new();
    let mut used = HashSet::new();
    collect_names(root, &masked, &mut declared, &mut used);
    let mut imports = BTreeSet::new();
    let mut span: Option<(usize, usize)> = None;
    for c in children(root).into_iter().filter(|c| c.kind() == "import_declaration") {
        span = Some(match span {
            Some((s, _)) => (s, c.end_byte()),
            None => (c.start_byte(), c.end_byte()),
        });
        let text = src[c.byte_range()].trim_start_matches("import").trim_end_matches(';').trim();
        if text.starts_with("static ") {
            imports.insert(text.to_string());
            continue;
        }
        match text.strip_suffix(".*") {
            Some(pkg) => match package_classes(pkg) {
                Some(classes) => {
                    for class in classes {
                        if used.contains(*class) && !declared.contains(*class) {
                            imports.insert(format!("{pkg}.{class}"));
                        }
                    }
                }
                None => {
                    imports.insert(text.to_string());
                }
            },
            None => {
                let simple = text.rsplit('.').next().unwrap_or(text);
                if used.contains(simple) {
                    imports.insert(text.to_string());
                }
            }
        }
    }
    let Some((start, end)) = span else {
        return src.to_string();
    };
    let block: Vec<String> = imports.iter().map(|i| format!("import {i};")).collect();
    let rest = src[end..].trim_start_matches('\n');
    let sep = if block.is_empty() { "" } else { "\n\n" };
    format!("{}{}{sep}{rest}", &src[..start], block.join("\n"))
}

fn collect_names(n: Node, src: &str, declared: &mut HashSet<String>, used: &mut HashSet<String>) {
    let text = |n: Node| src[n.byte_range()].to_string();
    match n.kind() {
        "import_declaration" | "package_declaration" => return,
        "class_declaration" | "record_declaration" | "interface_declaration"
        | "enum_declaration" => {
            if let Some(name) = n.child_by_field_name("name") {
                declared.insert(text(name));
            }
        }
        "type_parameter" => {
            if let Some(name) = n.named_child(0) {
                declared.insert(text(name));
            }
        }
        "type_identifier" => {
            // `Map.Entry`: only the outer name is imported.
            let inner = n.parent().is_some_and(|p| {
                p.kind() == "scoped_type_identifier" && p.named_child(0) != Some(n)
            });
            if !inner {
                used.insert(text(n));
            }
        }
        "identifier" => {
            let t = text(n);
            if t.starts_with(|c: char| c.is_ascii_uppercase()) {
                used.insert(t);
            }
        }
        _ => {}
    }
    for c in children(n) {
        collect_names(c, src, declared, used);
    }
}

fn package_classes(pkg: &str) -> Option<&'static [&'static str]> {
    Some(match pkg {
        "java.util" => &[
            "AbstractMap", "ArrayDeque", "ArrayList", "Arrays", "Base64", "BitSet", "Collection",
            "Collections", "Comparator", "ConcurrentModificationException", "Deque", "EnumMap",
            "EnumSet", "Enumeration", "Formatter", "HashMap", "HashSet", "Hashtable",
            "IdentityHashMap", "InputMismatchException", "Iterator", "LinkedHashMap",
            "LinkedHashSet", "LinkedList", "List", "ListIterator", "Locale", "Map",
            "NavigableMap", "NavigableSet", "NoSuchElementException", "Objects", "Optional",
            "OptionalDouble", "OptionalInt", "OptionalLong", "PrimitiveIterator",
            "PriorityQueue", "Queue", "Random", "Scanner", "SequencedCollection", "SequencedMap",
            "SequencedSet", "Set", "SortedMap", "SortedSet", "Spliterator", "Spliterators",
            "Stack", "StringJoiner", "StringTokenizer", "TreeMap", "TreeSet", "UUID", "Vector",
            "WeakHashMap",
        ],
        "java.util.function" => &[
            "BiConsumer", "BiFunction", "BiPredicate", "BinaryOperator", "BooleanSupplier",
            "Consumer", "DoubleBinaryOperator", "DoubleConsumer", "DoubleFunction",
            "DoublePredicate", "DoubleSupplier", "DoubleUnaryOperator", "Function",
            "IntBinaryOperator", "IntConsumer", "IntFunction", "IntPredicate", "IntSupplier",
            "IntToDoubleFunction", "IntToLongFunction", "IntUnaryOperator", "LongBinaryOperator",
            "LongConsumer", "LongFunction", "LongPredicate", "LongSupplier",
            "LongToIntFunction", "LongUnaryOperator", "ObjDoubleConsumer", "ObjIntConsumer",
            "ObjLongConsumer", "Predicate", "Supplier", "ToDoubleBiFunction",
            "ToDoubleFunction", "ToIntBiFunction", "ToIntFunction", "ToLongBiFunction",
            "ToLongFunction", "UnaryOperator",
        ],
        "java.util.stream" => &[
            "Collector", "Collectors", "DoubleStream", "IntStream", "LongStream", "Stream",
            "StreamSupport",
        ],
        "java.io" => &[
            "BufferedInputStream", "BufferedOutputStream", "BufferedReader", "BufferedWriter",
            "ByteArrayInputStream", "ByteArrayOutputStream", "Closeable", "DataInputStream",
            "EOFException", "File", "FileInputStream", "FileOutputStream", "FileReader",
            "FileWriter", "Flushable", "IOException", "InputStream", "InputStreamReader",
            "OutputStream", "OutputStreamWriter", "PrintStream", "PrintWriter", "Reader",
            "Serializable", "StreamTokenizer", "StringReader", "StringWriter",
            "UncheckedIOException", "Writer",
        ],
        "java.nio.file" => &[
            "AccessDeniedException", "DirectoryNotEmptyException", "DirectoryStream",
            "FileAlreadyExistsException", "FileSystems", "Files", "NoSuchFileException",
            "NotDirectoryException", "Path", "Paths", "StandardOpenOption",
        ],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(src: &str) -> String {
        relayout(src, &|_| false, &Hints::default()).unwrap()
    }

    #[test]
    fn splits_statements_and_members() {
        let src = "final class Main { static int a = 1; static void f() { int x = 1; if (x > 0) { g(x); }\n else { h(); } } }";
        assert_eq!(
            layout(src),
            "final class Main {\n    static int a = 1;\n\n    static void f() {\n        int x = 1;\n        if (x > 0) {\n            g(x);\n        } else {\n            h();\n        }\n    }\n}\n"
        );
    }

    #[test]
    fn fields_move_first_and_chains_rejoin() {
        let src = "class Main { static void f() { var s = xs.stream()\n   .map(y -> { return y; })\n   .toList(); } static int N = 3; }";
        assert_eq!(
            layout(src),
            "class Main {\n    static int N = 3;\n\n    static void f() {\n        var s = xs.stream().map(y -> {\n            return y;\n        }).toList();\n    }\n}\n"
        );
    }

    #[test]
    fn jdk_idioms() {
        let src = "class Main { static Optional<Integer> f(String l) { List<String> a = Arrays.stream(l.split(\",\")).toList(); List<String> b = Arrays.stream(l.split(\",\")).collect(Collectors.toCollection(ArrayList::new)); var c = a.stream().map(s -> Integer.parseInt(s)).toList(); boolean d = l.endsWith(String.valueOf('\\'')); Function<Integer, Optional<Integer>> g = x -> x > 0 ? Optional.of(x) : Optional.<Integer>empty(); return d ? Optional.of(1) : Optional.<Integer>empty(); } }";
        assert_eq!(
            layout(src),
            "class Main {\n    static Optional<Integer> f(String l) {\n        List<String> a = Arrays.asList(l.split(\",\"));\n        List<String> b = new ArrayList<>(Arrays.asList(l.split(\",\")));\n        var c = a.stream().map(Integer::parseInt).toList();\n        boolean d = l.endsWith(\"'\");\n        Function<Integer, Optional<Integer>> g = x -> x > 0 ? Optional.of(x) : Optional.<Integer>empty();\n        return d ? Optional.of(1) : Optional.empty();\n    }\n}\n"
        );
    }

    #[test]
    fn imports_become_explicit() {
        let src = "import java.io.*;\nimport java.util.*;\nimport java.util.regex.Pattern;\n\nclass Main { List<Map.Entry<String, Integer>> x = new ArrayList<>(); BufferedReader r; }\n";
        assert_eq!(
            explicit_imports(src),
            "import java.io.BufferedReader;\nimport java.util.ArrayList;\nimport java.util.List;\nimport java.util.Map;\n\nclass Main { List<Map.Entry<String, Integer>> x = new ArrayList<>(); BufferedReader r; }\n"
        );
    }
}
