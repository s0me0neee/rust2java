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
pub fn relayout(src: &str, blank: &dyn Fn(usize) -> bool) -> Option<String> {
    let masked = mask_qualified_calls(src)?;
    relayout_masked(&masked, blank).map(|s| s.replace(MASK, "."))
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

fn relayout_masked(src: &str, blank: &dyn Fn(usize) -> bool) -> Option<String> {
    let tree = parse(src)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    let f = Fmt {
        src,
        blank,
        decl_types: declared_types(root, src),
        enum_values_read: reads_value_field(root, src),
    };
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
        if n.child_count() == 0 || ATOMS.contains(&n.kind()) {
            return self.text(n).trim_end().to_string();
        }
        let plain = self.plain(n);
        if plain != n {
            return self.render(plain, ind);
        }
        let mut out = String::new();
        let mut prev: Option<Node> = None;
        for c in children(n) {
            if let Some(p) = prev {
                out.push_str(&self.gap(p, c, ind));
            }
            out.push_str(&self.render(c, ind));
            prev = Some(c);
        }
        out
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
        for &c in items {
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
        relayout(src, &|_| false).unwrap()
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
    fn imports_become_explicit() {
        let src = "import java.io.*;\nimport java.util.*;\nimport java.util.regex.Pattern;\n\nclass Main { List<Map.Entry<String, Integer>> x = new ArrayList<>(); BufferedReader r; }\n";
        assert_eq!(
            explicit_imports(src),
            "import java.io.BufferedReader;\nimport java.util.ArrayList;\nimport java.util.List;\nimport java.util.Map;\n\nclass Main { List<Map.Entry<String, Integer>> x = new ArrayList<>(); BufferedReader r; }\n"
        );
    }
}
