// Rust syntax tree (+ semantics) -> JIR. One submodule per construct family.

// Generated lambda parameters (`x`, or `p = "parts"`), free again for the next lambda once named.
macro_rules! lambda_params {
    ($cx:expr; $($v:ident $(= $base:literal)?),+) => {
        $(let $v = $cx.fresh_param(lambda_params!(@base $v $($base)?));)+
        $($cx.release_param(&$v);)+
    };
    (@base $v:ident $base:literal) => { $base };
    (@base $v:ident) => { stringify!($v) };
}

pub mod builders;
pub mod calls;
pub mod expr;
pub mod fmt;
pub mod generics;
pub mod indexed;
pub mod items;
pub mod iter;
pub mod macros;
pub mod mutret;
pub mod pat;
pub mod std_map;
pub mod stmt;
pub mod throws;
pub mod traits;
pub mod tuples;
pub mod values;
pub mod views;
pub mod wide;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use ra_ap_hir::{Module, Semantics};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, ast};

use crate::frontend::{self, Unit};
use crate::jir::{self, Expr, JavaFile};
use crate::ty::Ty;

#[derive(Debug, Clone)]
pub struct Diag {
    pub line: u32,
    pub col: u32,
    pub msg: String,
}

pub struct Cx<'a, 'db> {
    pub sema: &'a Semantics<'db, RootDatabase>,
    pub db: &'db RootDatabase,
    pub module: Module,
    pub class_name: String,
    // Java name of `Self` while lowering an impl/trait body (defaults to the module class).
    pub self_ty_java: String,
    // What `self` means in the body being lowered (`this`, or a parameter of a static method).
    pub self_name: String,
    // Inside an `impl Display` body: `?` on `write!` is dropped.
    pub in_display: bool,
    line_starts: Vec<u32>,
    pub imports: BTreeSet<String>,
    pub diags: Vec<Diag>,
    // Correct output, but worth knowing (e.g. an operation slower than in Rust).
    pub warnings: Vec<Diag>,
    // Lexical scopes of the current method: Rust name -> Java name.
    scopes: Vec<HashMap<String, String>>,
    // Temporary name aliases: Rust name -> already-rendered Java text (pattern bindings in guards).
    aliases: Vec<HashMap<String, String>>,
    // Java names declared anywhere in the current method (Java forbids shadowing locals).
    method_names: HashSet<String>,
    tmp_counter: u32,
    // Declared return type of the function being lowered.
    pub ret_ty: Option<Ty>,
    // Code appended to the file's last line (entry point, generated records, fields).
    pub file_trailer: Vec<String>,
    // `STDIN` was declared above a static initializer that reads it, not on the last line.
    pub stdin_hoisted: bool,
    // Set when the program uses `Result`, so `Result.java` gets written.
    pub needs_result: bool,
    // Enclosing loops/labeled blocks: (Java label, variable receiving `break value`).
    pub loops: Vec<LoopFrame>,
    // Number of enclosing Java switch statements (affects unlabeled `break`).
    pub switch_depth: u32,
    // Locals stored in one-element arrays (`int[] n = {0}`): `&mut` targets and closure-mutated vars.
    pub cells: HashSet<ra_ap_hir::Local<'db>>,
    // `String` locals appended to in a loop, held in a `StringBuilder`.
    pub builders: HashSet<ra_ap_hir::Local<'db>>,
    // `&str` locals kept as (base, start offset); see `views`.
    pub views: HashMap<ra_ap_hir::Local<'db>, views::View>,
    // Struct/array types changed in place somewhere in the crate (computed on first use; see `values`).
    pub mutated: Option<values::Mutated>,
    pub wide: wide::Wide<'db>,
    // `(trait, const)` pairs read through a type parameter or `Self` (computed on first use).
    pub generic_consts: Option<HashSet<(String, String)>>,
    // The statement-level call whose `&mut field` arguments may use a temporary cell, and the
    // write-backs that must follow that statement.
    pub write_back_call: Option<SyntaxNode>,
    pub write_backs: Vec<jir::Stmt>,
    // Lambda parameter names used in the current statement (see `release_param`).
    pub stmt_lambda_names: HashSet<String>,
    param_mode: bool,
    // Locals that *are* a one-element array: `Cell<i32>`/`RefCell<i32>` handles (`int[] c = {0}`).
    pub cell_handles: HashSet<ra_ap_hir::Local<'db>>,
    // >0 while lowering a closure body: statements hoisted to `out` would run only once, outside it.
    pub lambda_depth: u32,
    // Iterator locals pulled with `next()`/`peek()`: Java `ListIterator` + the backing list's name.
    pub pulled_iters: HashMap<ra_ap_hir::Local<'db>, String>,
    // `for x in v.iter_mut()` over immutable values: `x` stands for `list.get(idx)`.
    pub elem_aliases: HashMap<ra_ap_hir::Local<'db>, (String, String, ElemAliasKind)>,
    // Set while lowering the closure of an `IntStream.range` chain: what its `(a, b)` fields read.
    pub indexed_params: Option<Vec<indexed::Item>>,
    // The chain's index: the Rust name a stage gives it, then the Java name every stage shares.
    pub indexed_index: (Option<String>, Option<String>),
    // Local ADTs implementing Iterator/Deref, recorded for their use sites.
    pub local_traits: HashMap<String, traits::TraitImpls>,
    // macro_rules! arms lowered as static methods: macro name -> Java method name.
    pub macro_methods: HashMap<String, String>,
    // Those methods, placed among the class members by line when the file is done.
    pub macro_method_defs: Vec<crate::jir::Method>,
    // While lowering such a method: expansion tree root and expansion offset -> definition line.
    pub macro_lines: Option<(SyntaxNode, HashMap<u32, u32>)>,
    // `env::args()` was used, so the entry point keeps the command-line arguments.
    pub uses_args: bool,
    // No byte-level reads anywhere in the file: stdin is a `BufferedReader` read by lines.
    pub stdin_lines: bool,
    // The array literal being lowered initializes a binding that is only ever read: `List.of`.
    pub read_only_literal: bool,
    // `let x = <init>;` where lowering `init` ends in a fresh local (`?`, `parse`): that local is
    // declared as `x` itself. Set by the let, taken via `take_let_target`.
    pub try_let: Option<(SyntaxNode, String)>,
    pub try_let_declared: Option<String>,
    // A name chosen by the caller for the next parse result (see `result_name`).
    pub pending_name: Option<String>,
    // Simple mode: Java names of types whose derived Debug is their `toString()` (no Display).
    pub debug_to_string: HashSet<String>,
    // Simple mode: functions returning their `&mut` primitive parameter (see `mutret`).
    pub mut_returns: HashMap<ra_ap_hir::Function, usize>,
    // Simple mode: `&mut String` parameters and the locals handed to them, as `StringBuilder`s.
    pub builder_params: HashSet<(ra_ap_hir::Function, usize)>,
    pub builder_locals: HashSet<ra_ap_hir::Local<'db>>,
    // Functions needing a `Supplier<T>` per type parameter (`T::default()`), and this method's.
    pub supplier_fns: HashMap<ra_ap_hir::Function, Vec<String>>,
    pub suppliers: HashMap<String, String>,
    // Their parameters, while lowering those functions: `*n` is just `n`.
    pub returned_params: HashSet<ra_ap_hir::Local<'db>>,
    // Every identifier in the Rust file: generated names never take one of these.
    file_idents: HashSet<String>,
    // Java names of `let` locals declared inside macro expansions, and outside them.
    pub macro_locals: HashSet<String>,
    pub user_locals: HashSet<String>,
    // User locals renamed for a clash (`s3` for Rust `s`): Java name, Rust name.
    pub renamed_locals: Vec<(String, String)>,
}

// Per-method state saved around lowering a nested method (a macro method generated mid-body).
pub struct NestedState<'db> {
    method: MethodState,
    tmp_counter: u32,
    cells: HashSet<ra_ap_hir::Local<'db>>,
    cell_handles: HashSet<ra_ap_hir::Local<'db>>,
    pulled_iters: HashMap<ra_ap_hir::Local<'db>, String>,
    elem_aliases: HashMap<ra_ap_hir::Local<'db>, (String, String, ElemAliasKind)>,
    lambda_depth: u32,
    in_display: bool,
    macro_lines: Option<(SyntaxNode, HashMap<u32, u32>)>,
}

#[derive(Clone, Copy)]
pub enum ElemAliasKind {
    List,
    Map,
    Entry,
    // A string's char, `s.charAt(i)`: read-only.
    Char,
}

#[derive(Clone, Debug)]
pub struct LoopFrame {
    pub rust_label: Option<String>,
    pub java_label: Option<String>,
    // Where `break value` sends the value (a local, or the method's return).
    pub value_dest: Option<stmt::Dest>,
    // `cx.switch_depth` when the loop was entered.
    pub switch_depth: u32,
    // Labeled block rather than a loop (unlabeled `break` skips it).
    pub is_block: bool,
}

// Method-scoped state, saved while lowering a nested item (a local `fn`).
pub struct MethodState {
    scopes: Vec<HashMap<String, String>>,
    method_names: HashSet<String>,
    ret_ty: Option<Ty>,
    loops: Vec<LoopFrame>,
    switch_depth: u32,
}

pub struct Lowered {
    pub file: JavaFile,
    pub comments: BTreeMap<u32, Vec<String>>,
    pub trailer: String,
    pub last_line: u32,
    pub diags: Vec<Diag>,
    pub warnings: Vec<Diag>,
    pub needs_result: bool,
    // 1-based Rust lines that are empty (simple mode keeps them as blank lines).
    pub blank_lines: HashSet<u32>,
    // Simple mode: macro locals to inline, and clash renames to undo.
    pub hints: crate::javafmt::Hints,
}

// Names each non-std dependency once, at its first use, so later per-call errors have a cause.
fn report_external_crates(cx: &mut Cx, root: &SyntaxNode) {
    let mut seen = HashSet::new();
    for path in root.descendants().filter_map(ast::Path::cast) {
        if path.qualifier().is_some() {
            continue;
        }
        let Some(ra_ap_hir::PathResolution::Def(ra_ap_hir::ModuleDef::Module(m))) =
            cx.sema.resolve_path(&path)
        else {
            continue;
        };
        let krate = m.krate(cx.db);
        if !m.is_crate_root(cx.db) || !krate.origin(cx.db).is_lib() {
            continue;
        }
        let name = krate
            .display_name(cx.db)
            .map(|n| n.to_string())
            .unwrap_or_else(|| path.syntax().text().to_string());
        if seen.insert(name.clone()) {
            cx.error(
                path.syntax(),
                format!("external crate `{name}` is not supported (only std)"),
            );
        }
    }
}

pub fn lower_unit(sema: &Semantics<'_, RootDatabase>, unit: &Unit) -> Lowered {
    let file = sema.parse(unit.file_id);
    crate::tuples::clear();
    let text = file.syntax().text().to_string();
    let mut line_starts = vec![0u32];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i as u32 + 1);
        }
    }
    let mut cx = Cx {
        sema,
        db: sema.db,
        module: unit.module,
        class_name: unit.class_name.clone(),
        self_ty_java: unit.class_name.clone(),
        self_name: "this".into(),
        in_display: false,
        line_starts,
        imports: BTreeSet::new(),
        diags: Vec::new(),
        warnings: Vec::new(),
        scopes: Vec::new(),
        aliases: Vec::new(),
        method_names: HashSet::new(),
        tmp_counter: 0,
        ret_ty: None,
        file_trailer: Vec::new(),
        stdin_hoisted: false,
        needs_result: false,
        loops: Vec::new(),
        switch_depth: 0,
        cells: HashSet::new(),
        builders: HashSet::new(),
        views: HashMap::new(),
        mutated: None,
        wide: wide::Wide {
            locals: HashSet::new(),
            ops: HashSet::new(),
        },
        generic_consts: None,
        write_back_call: None,
        write_backs: Vec::new(),
        stmt_lambda_names: HashSet::new(),
        param_mode: false,
        cell_handles: HashSet::new(),
        lambda_depth: 0,
        pulled_iters: HashMap::new(),
        elem_aliases: HashMap::new(),
        indexed_params: None,
        indexed_index: (None, None),
        local_traits: HashMap::new(),
        macro_methods: HashMap::new(),
        macro_method_defs: Vec::new(),
        macro_lines: None,
        uses_args: false,
        stdin_lines: false,
        read_only_literal: false,
        try_let: None,
        try_let_declared: None,
        pending_name: None,
        debug_to_string: HashSet::new(),
        mut_returns: HashMap::new(),
        builder_params: HashSet::new(),
        supplier_fns: HashMap::new(),
        suppliers: HashMap::new(),
        builder_locals: HashSet::new(),
        returned_params: HashSet::new(),
        macro_locals: HashSet::new(),
        user_locals: HashSet::new(),
        renamed_locals: Vec::new(),
        // Names a later Rust binding (or an unqualified const/static) will claim; function, type
        // and field names live in other Java namespaces.
        file_idents: file
            .syntax()
            .descendants()
            .filter_map(|n| {
                use ast::HasName;
                if let Some(ip) = ast::IdentPat::cast(n.clone()) {
                    ip.name()
                } else if let Some(c) = ast::Const::cast(n.clone()) {
                    c.name()
                } else {
                    ast::Static::cast(n).and_then(|s| s.name())
                }
            })
            .map(|n| n.text().to_string())
            // Macro bodies and arguments are token trees: their bindings aren't IdentPats.
            .chain(
                file.syntax()
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| {
                        t.kind() == SyntaxKind::IDENT
                            && t.parent_ancestors()
                                .any(|a| a.kind() == SyntaxKind::TOKEN_TREE)
                    })
                    .map(|t| t.text().to_string()),
            )
            .collect(),
    };
    report_external_crates(&mut cx, file.syntax());
    cx.stdin_lines = !std_map::io::has_byte_reads(&cx, file.syntax());
    if crate::mode::simple() {
        crate::mode::set_throws(true);
        let verdict = throws::eligible(&cx, file.syntax());
        crate::mode::set_throws(verdict.is_ok());
        if let Err((node, why)) = verdict {
            let text = node.text().to_string();
            let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
            let short: String = text.chars().take(48).collect();
            let more = if short.len() < text.len() { "…" } else { "" };
            cx.warn(
                &node,
                format!("Result kept as a value (not exceptions) in this file: {why}: `{short}{more}`"),
            );
        }
    }
    cx.wide = wide::analyze(&cx, file.syntax());
    // Both modes: `T::default()` needs a `Supplier<T>` handed down from the call sites.
    generics::analyze(&mut cx, file.syntax());
    calls::dump_std_calls(&cx, file.syntax());
    if crate::mode::simple() {
        cx.debug_to_string = items::debug_to_string_types(file.syntax());
        cx.mut_returns = mutret::analyze(&cx, file.syntax());
        let b = builders::analyze(&cx, file.syntax());
        cx.builder_params = b.params;
        cx.builder_locals = b.locals;
        if crate::tuples::enabled() {
            tuples::analyze(&mut cx, file.syntax());
        }
    }
    let comments = collect_comments(&cx, file.syntax());
    let last_line = text.trim_end().lines().count().max(1) as u32;
    let class = items::lower_source_file(&mut cx, &file, unit.is_root);
    for imp in ["java.util.*", "java.util.function.*", "java.util.stream.*"] {
        cx.imports.insert(imp.to_string());
    }
    let jf = JavaFile {
        package: unit.package.clone(),
        imports: cx.imports.iter().cloned().collect(),
        class,
    };
    let mut renamed_locals: HashMap<String, Option<String>> = HashMap::new();
    for (java, rust) in &cx.renamed_locals {
        let e = renamed_locals.entry(java.clone()).or_insert(Some(rust.clone()));
        if e.as_ref() != Some(rust) {
            *e = None;
        }
    }
    Lowered {
        hints: crate::javafmt::Hints {
            macro_locals: cx.macro_locals.difference(&cx.user_locals).cloned().collect(),
            renamed_locals: renamed_locals.into_iter().filter_map(|(j, r)| Some((j, r?))).collect(),
        },
        file: jf,
        comments,
        trailer: cx.file_trailer.join(" "),
        last_line,
        diags: cx.diags,
        warnings: cx.warnings,
        needs_result: cx.needs_result,
        blank_lines: text
            .lines()
            .enumerate()
            .filter(|(_, l)| l.trim().is_empty())
            .map(|(i, _)| i as u32 + 1)
            .collect(),
    }
}

fn collect_comments(cx: &Cx, root: &SyntaxNode) -> BTreeMap<u32, Vec<String>> {
    let mut map: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for tok in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if tok.kind() == SyntaxKind::COMMENT {
            let line = cx.line_of_offset(tok.text_range().start().into());
            map.entry(line).or_default().push(tok.text().to_string());
        }
    }
    map
}

impl<'a, 'db> Cx<'a, 'db> {
    pub fn line_of_offset(&self, off: u32) -> u32 {
        match self.line_starts.binary_search(&off) {
            Ok(i) => i as u32 + 1,
            Err(i) => i as u32,
        }
    }

    // 1-based line where `node` starts, ignoring leading comments/attributes-trivia
    // (macro-expanded nodes map to their call site).
    pub fn line(&self, node: &SyntaxNode) -> u32 {
        if let Some(l) = self.macro_line(node, false) {
            return l;
        }
        let first = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| !t.kind().is_trivia());
        let range = self.sema.original_range(node).range;
        let start: u32 = match first {
            Some(t) if range == node.text_range() => t.text_range().start().into(),
            _ => range.start().into(),
        };
        self.line_of_offset(start)
    }

    // Inside a macro method: the line in the macro definition the node (or the macro call
    // containing it) came from.
    fn macro_line(&self, node: &SyntaxNode, last: bool) -> Option<u32> {
        let (root, map) = self.macro_lines.as_ref()?;
        let anc = self
            .sema
            .ancestors_with_macros(node.clone())
            .find(|a| a.ancestors().last().as_ref() == Some(root))?;
        let mut tokens = anc
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !t.kind().is_trivia());
        let t = if last { tokens.last() } else { tokens.next() }?;
        map.get(&u32::from(t.text_range().start())).copied()
    }

    pub fn end_line(&self, node: &SyntaxNode) -> u32 {
        if let Some(l) = self.macro_line(node, true) {
            return l;
        }
        let range = self.sema.original_range(node).range;
        self.line_of_offset(u32::from(range.end()).saturating_sub(1))
    }

    pub fn col(&self, node: &SyntaxNode) -> u32 {
        let off: u32 = self.sema.original_range(node).range.start().into();
        let line = self.line_of_offset(off);
        off - self.line_starts[(line - 1) as usize] + 1
    }

    pub fn convert(&self, t: &ra_ap_hir::Type<'db>) -> Ty {
        frontend::convert(self.sema, self.module, t)
    }

    // Type of an expression before implicit coercions.
    pub fn ty(&self, e: &ast::Expr) -> Ty {
        match self.sema.type_of_expr(e) {
            Some(info) => self.convert(&info.original),
            None => Ty::Other("{unknown}".into()),
        }
    }

    // Type after coercions (what the surrounding context receives).
    pub fn adjusted_ty(&self, e: &ast::Expr) -> Ty {
        match self.sema.type_of_expr(e) {
            Some(info) => self.convert(&info.adjusted()),
            None => Ty::Other("{unknown}".into()),
        }
    }

    pub fn pat_ty(&self, p: &ast::Pat) -> Ty {
        match self.sema.type_of_pat(p) {
            Some(info) => self.convert(&info.original),
            None => Ty::Other("{unknown}".into()),
        }
    }

    pub fn type_ref(&self, t: &ast::Type) -> Ty {
        match self.sema.resolve_type(t) {
            Some(t) => self.convert(&t),
            None => Ty::Other(t.syntax().text().to_string()),
        }
    }

    // Records an unsupported construct; the placeholder fails javac so the gap can't ship silently.
    pub fn unsupported(&mut self, node: &SyntaxNode, what: &str) -> Expr {
        let line = self.line(node);
        let col = self.col(node);
        self.diags.push(Diag {
            line,
            col,
            msg: format!("unsupported: {what}"),
        });
        Expr::raw(format!("/* unsupported: {what} */ UNSUPPORTED"))
    }

    pub fn warn(&mut self, node: &SyntaxNode, msg: impl Into<String>) {
        let line = self.line(node);
        let col = self.col(node);
        self.warnings.push(Diag {
            line,
            col,
            msg: msg.into(),
        });
    }

    pub fn error(&mut self, node: &SyntaxNode, msg: impl Into<String>) {
        let line = self.line(node);
        let col = self.col(node);
        self.diags.push(Diag {
            line,
            col,
            msg: msg.into(),
        });
    }

    // --- naming -------------------------------------------------------------------------

    pub fn enter_method(&mut self) {
        self.scopes = vec![HashMap::new()];
        self.method_names.clear();
        self.tmp_counter = 0;
        self.cells.clear();
        self.stmt_lambda_names.clear();
        self.builders.clear();
        self.suppliers.clear();
        self.views.clear();
        self.cell_handles.clear();
        self.pulled_iters.clear();
        self.elem_aliases.clear();
    }

    pub fn local_of_path(&self, e: &ast::Expr) -> Option<ra_ap_hir::Local<'db>> {
        let ast::Expr::PathExpr(pe) = strip_parens(e) else {
            return None;
        };
        match self.sema.resolve_path(&pe.path()?) {
            Some(ra_ap_hir::PathResolution::Local(l)) => Some(l),
            _ => None,
        }
    }

    // --- cells (`&mut` to Java-immutable values) --------------------------------------------

    // Whether a `let` binding must live in a one-element array: its value type is immutable in
    // Java and it is either borrowed `&mut` or (being `mut`) captured by a closure.
    pub fn binding_needs_cell(&self, ip: &ast::IdentPat, ty: &Ty) -> bool {
        if !is_mut_ref_cell_ty(ty) {
            return false;
        }
        let Some(local) = self.sema.to_def(ip) else {
            return false;
        };
        // A shared `StringBuilder` is already the cell.
        if self.builder_locals.contains(&local) {
            return false;
        }
        let Some(scope) = self.enclosing_fn(ip.syntax()) else {
            return false;
        };
        let is_mut = ip.mut_token().is_some();
        for pe in self
            .expanded_descendants(&scope)
            .into_iter()
            .filter_map(ast::PathExpr::cast)
        {
            let Some(path) = pe.path() else { continue };
            if !matches!(self.sema.resolve_path(&path), Some(ra_ap_hir::PathResolution::Local(l)) if l == local)
            {
                continue;
            }
            let borrowed_mut = pe
                .syntax()
                .parent()
                .and_then(ast::RefExpr::cast)
                .is_some_and(|r| {
                    r.mut_token().is_some()
                        && !is_inline_read_arg(&r)
                        && !mutret::is_returned_arg(self, &r)
                });
            let in_closure = pe
                .syntax()
                .ancestors()
                .take_while(|n| *n != scope)
                .filter_map(ast::ClosureExpr::cast)
                .any(|c| {
                    !c.syntax()
                        .text_range()
                        .contains_range(ip.syntax().text_range())
                        && !iter::closure_is_loop(self, &c)
                });
            if borrowed_mut || (is_mut && in_closure) {
                return true;
            }
        }
        false
    }

    // A `let mut s = String..` appended to inside a loop: Java `+=` would copy the whole string
    // per append, so it becomes a `StringBuilder`. Only appends, `clear`, `len`, `is_empty` and
    // reassignment touch the builder; every other use reads `s.toString()`.
    pub fn binding_is_builder(&self, ip: &ast::IdentPat) -> bool {
        if ip.mut_token().is_none() || ip.ref_token().is_some() {
            return false;
        }
        let is_string = self
            .sema
            .type_of_pat(&ast::Pat::IdentPat(ip.clone()))
            .and_then(|t| t.original.as_adt())
            .is_some_and(|a| a.name(self.db).as_str() == "String");
        let (Some(local), Some(scope)) = (self.sema.to_def(ip), self.enclosing_fn(ip.syntax()))
        else {
            return false;
        };
        if self.builder_locals.contains(&local) {
            return true;
        }
        if !is_string {
            return false;
        }
        let Some(home) = ip.syntax().ancestors().find_map(ast::StmtList::cast) else {
            return false;
        };
        let mut appended_in_loop = false;
        for pe in self
            .expanded_descendants(&scope)
            .into_iter()
            .filter_map(ast::PathExpr::cast)
        {
            let Some(path) = pe.path() else { continue };
            if !matches!(self.sema.resolve_path(&path), Some(ra_ap_hir::PathResolution::Local(l)) if l == local)
            {
                continue;
            }
            let ancestors: Vec<SyntaxNode> = self
                .sema
                .ancestors_with_macros(pe.syntax().clone())
                .skip(1)
                .take_while(|n| n != home.syntax())
                .collect();
            if ancestors
                .iter()
                .any(|n| ast::ClosureExpr::can_cast(n.kind()))
            {
                return false;
            }
            // Look through `&mut s` / `(s)` to the expression using the string.
            let mut user = pe.syntax().clone();
            let mut via_mut_ref = false;
            while let Some(parent) = user.parent() {
                if let Some(r) = ast::RefExpr::cast(parent.clone()) {
                    via_mut_ref |= r.mut_token().is_some();
                } else if !ast::ParenExpr::can_cast(parent.kind()) {
                    break;
                }
                user = parent;
            }
            let parent = user.parent();
            let method = parent
                .clone()
                .and_then(ast::MethodCallExpr::cast)
                .filter(|m| m.receiver().is_some_and(|r| r.syntax() == &user))
                .and_then(|m| m.name_ref())
                .map(|n| n.text().to_string());
            let compound = parent.clone().and_then(ast::BinExpr::cast).filter(|b| {
                b.lhs().is_some_and(|l| l.syntax() == &user)
                    && b.op_kind()
                        .is_some_and(|op| matches!(op, ast::BinaryOp::Assignment { .. }))
            });
            let append = match (&method, &compound) {
                (Some(m), _)
                    if matches!(m.as_str(), "push" | "push_str" | "write_fmt" | "write_str") =>
                {
                    true
                }
                (Some(m), _) if matches!(m.as_str(), "clear" | "len" | "is_empty") => false,
                // Other `&mut self` methods (pop, insert, truncate, ..) would need the String.
                (Some(m), _) if self.mutating_str_method(m) => return false,
                (_, Some(b)) => match b.op_kind() {
                    Some(ast::BinaryOp::Assignment {
                        op: Some(ast::ArithOp::Add),
                    }) => true,
                    Some(ast::BinaryOp::Assignment { op: None }) => false,
                    _ => return false,
                },
                _ if via_mut_ref => return false,
                _ => false,
            };
            if append
                && ancestors.iter().any(|n| {
                    ast::ForExpr::can_cast(n.kind())
                        || ast::WhileExpr::can_cast(n.kind())
                        || ast::LoopExpr::can_cast(n.kind())
                })
            {
                appended_in_loop = true;
            }
        }
        appended_in_loop
    }

    fn mutating_str_method(&self, name: &str) -> bool {
        matches!(
            name,
            "pop"
                | "insert"
                | "insert_str"
                | "remove"
                | "retain"
                | "truncate"
                | "drain"
                | "extend"
                | "replace_range"
                | "make_ascii_uppercase"
                | "make_ascii_lowercase"
                | "split_off"
                | "reserve"
                | "shrink_to_fit"
                | "as_mut_str"
        )
    }

    pub fn view_of(&self, e: &ast::Expr) -> Option<views::View> {
        self.local_of_path(e)
            .and_then(|l| self.views.get(&l).cloned())
    }

    // Java name of the `StringBuilder` if `e` is a path to a builder local.
    pub fn builder_path(&self, e: &ast::Expr) -> Option<String> {
        // `*out` of a builder parameter is the builder too.
        let e = match strip_parens(e) {
            ast::Expr::PrefixExpr(p) if p.op_kind() == Some(ast::UnaryOp::Deref) => p.expr()?,
            other => other,
        };
        let ast::Expr::PathExpr(pe) = strip_parens(&e) else {
            return None;
        };
        match self.sema.resolve_path(&pe.path()?) {
            Some(ra_ap_hir::PathResolution::Local(l)) if self.builders.contains(&l) => {
                Some(self.lookup(l.name(self.db).as_str()))
            }
            _ => None,
        }
    }

    // An immutable binding whose value is only read (method receiver, `&x`, `x[i]`, `for .. in x`)
    // and never moved, so it can hold an unmodifiable Java list.
    pub fn binding_read_only(&self, ip: &ast::IdentPat) -> bool {
        if ip.mut_token().is_some() || ip.ref_token().is_some() {
            return false;
        }
        let Some(local) = self.sema.to_def(ip) else {
            return false;
        };
        let Some(scope) = self.enclosing_fn(ip.syntax()) else {
            return false;
        };
        self.expanded_descendants(&scope)
            .into_iter()
            .filter_map(ast::PathExpr::cast)
            .filter(|pe| {
                pe.path().is_some_and(|path| {
                    matches!(self.sema.resolve_path(&path), Some(ra_ap_hir::PathResolution::Local(l)) if l == local)
                })
            })
            .all(|pe| {
                let me = pe.syntax().clone();
                let Some(parent) = me.parent() else {
                    return false;
                };
                if let Some(r) = ast::RefExpr::cast(parent.clone()) {
                    return r.mut_token().is_none();
                }
                if let Some(m) = ast::MethodCallExpr::cast(parent.clone()) {
                    return m.receiver().is_some_and(|r| r.syntax() == &me);
                }
                if let Some(i) = ast::IndexExpr::cast(parent.clone()) {
                    return i.base().is_some_and(|b| b.syntax() == &me);
                }
                // A `{:?}` argument is only streamed by the format helper.
                if ast::FormatArgsArg::can_cast(parent.kind()) {
                    return true;
                }
                if let Some(f) = ast::ForExpr::cast(parent) {
                    return f.iterable().is_some_and(|it| it.syntax() == &me);
                }
                false
            })
    }

    // Whether any closure (not containing the binding itself) refers to this binding.
    pub fn binding_captured(&self, ip: &ast::IdentPat) -> bool {
        let Some(local) = self.sema.to_def(ip) else {
            return false;
        };
        let Some(scope) = self.enclosing_fn(ip.syntax()) else {
            return false;
        };
        self.expanded_descendants(&scope).into_iter().filter_map(ast::ClosureExpr::cast).any(|c| {
            !c.syntax().text_range().contains_range(ip.syntax().text_range())
                && self.expanded_descendants(c.syntax()).into_iter().filter_map(ast::PathExpr::cast).any(|pe| {
                    pe.path().is_some_and(|path| {
                        matches!(self.sema.resolve_path(&path), Some(ra_ap_hir::PathResolution::Local(l)) if l == local)
                    })
                })
        })
    }

    // The `fn` around `node`, looking through macro expansions to their call sites.
    pub fn enclosing_fn(&self, node: &SyntaxNode) -> Option<SyntaxNode> {
        self.sema
            .ancestors_with_macros(node.clone())
            .find(|n| ast::Fn::can_cast(n.kind()))
    }

    // All nodes under `root`, including the expansions of macro calls (whose arguments are only
    // token trees in the original file).
    pub fn expanded_descendants(&self, root: &SyntaxNode) -> Vec<SyntaxNode> {
        let mut out = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(n) = stack.pop() {
            for d in n.descendants() {
                if let Some(mc) = ast::MacroCall::cast(d.clone())
                    && let Some(exp) = self.sema.expand_macro_call(&mc)
                {
                    stack.push(exp.value);
                }
                out.push(d);
            }
        }
        out
    }

    pub fn mark_cell(&mut self, ip: &ast::IdentPat) {
        if let Some(local) = self.sema.to_def(ip) {
            self.cells.insert(local);
        }
    }

    // Marks `x` in `let x: Cell<i32>` as naming the one-element array itself (not its element).
    pub fn mark_cell_handle(&mut self, ip: &ast::IdentPat) {
        if let Some(local) = self.sema.to_def(ip) {
            self.cell_handles.insert(local);
        }
    }

    // Java name of the array if `e` is a path to a `Cell`/`RefCell` handle local.
    pub fn handle_path(&self, e: &ast::Expr) -> Option<String> {
        let ast::Expr::PathExpr(pe) = e else {
            return None;
        };
        match self.sema.resolve_path(&pe.path()?) {
            Some(ra_ap_hir::PathResolution::Local(l)) if self.cell_handles.contains(&l) => {
                Some(self.lookup(l.name(self.db).as_str()))
            }
            _ => None,
        }
    }

    // Java name of the cell array if `e` is a path to a cell local.
    pub fn cell_path(&self, e: &ast::Expr) -> Option<String> {
        let ast::Expr::PathExpr(pe) = e else {
            return None;
        };
        match self.sema.resolve_path(&pe.path()?) {
            Some(ra_ap_hir::PathResolution::Local(l)) if self.cells.contains(&l) => {
                Some(self.lookup(l.name(self.db).as_str()))
            }
            _ => None,
        }
    }

    pub fn save_method_state(&mut self) -> MethodState {
        MethodState {
            scopes: std::mem::take(&mut self.scopes),
            method_names: std::mem::take(&mut self.method_names),
            ret_ty: self.ret_ty.take(),
            loops: std::mem::take(&mut self.loops),
            switch_depth: std::mem::replace(&mut self.switch_depth, 0),
        }
    }

    pub fn save_nested_state(&mut self) -> NestedState<'db> {
        let method = self.save_method_state();
        NestedState {
            method,
            tmp_counter: self.tmp_counter,
            cells: std::mem::take(&mut self.cells),
            cell_handles: std::mem::take(&mut self.cell_handles),
            pulled_iters: std::mem::take(&mut self.pulled_iters),
            elem_aliases: std::mem::take(&mut self.elem_aliases),
            lambda_depth: std::mem::replace(&mut self.lambda_depth, 0),
            in_display: std::mem::replace(&mut self.in_display, false),
            macro_lines: self.macro_lines.take(),
        }
    }

    pub fn restore_nested_state(&mut self, saved: NestedState<'db>) {
        self.restore_method_state(saved.method);
        self.tmp_counter = saved.tmp_counter;
        self.cells = saved.cells;
        self.cell_handles = saved.cell_handles;
        self.pulled_iters = saved.pulled_iters;
        self.elem_aliases = saved.elem_aliases;
        self.lambda_depth = saved.lambda_depth;
        self.in_display = saved.in_display;
        self.macro_lines = saved.macro_lines;
    }

    pub fn restore_method_state(&mut self, saved: MethodState) {
        self.scopes = saved.scopes;
        self.method_names = saved.method_names;
        self.ret_ty = saved.ret_ty;
        self.loops = saved.loops;
        self.switch_depth = saved.switch_depth;
    }

    // Names taken so far in the current method; a lambda's own names can be released after it,
    // since Java only forbids lambda names that shadow enclosing locals.
    pub fn taken_names(&self) -> HashSet<String> {
        self.method_names.clone()
    }

    // After a lambda is built, its names are free for sibling lambdas; locals of the same
    // statement (hoisted before it) still avoid them.
    pub fn release_names_since(&mut self, before: &HashSet<String>) {
        let added: Vec<String> = self.method_names.difference(before).cloned().collect();
        for n in added {
            self.release_param(&n);
        }
    }

    // A generated lambda parameter: avoids locals and enclosing lambdas' parameters only.
    // The `let` binding that `node` (the let's initializer) may declare directly.
    pub fn take_let_target(&mut self, node: &SyntaxNode) -> Option<String> {
        match &self.try_let {
            Some((n, _)) if n == node => {}
            _ => return None,
        }
        let (_, rust) = self.try_let.take()?;
        let java = self.declare(&rust);
        self.try_let_declared = Some(java.clone());
        Some(java)
    }

    // The local holding a parse result: the pending `let` name, else a fresh `result`.
    pub fn result_name(&mut self) -> String {
        self.pending_name
            .take()
            .unwrap_or_else(|| self.fresh("res"))
    }

    pub fn fresh_param(&mut self, base: &str) -> String {
        self.param_mode = true;
        let n = self.fresh(base);
        self.param_mode = false;
        n
    }

    pub fn declare_param(&mut self, rust: &str) -> String {
        self.param_mode = true;
        let n = self.declare(rust);
        self.param_mode = false;
        n
    }

    pub fn release_param(&mut self, name: &str) {
        self.method_names.remove(name);
        self.stmt_lambda_names.insert(name.to_string());
    }

    fn name_taken(&self, name: &str) -> bool {
        self.method_names.contains(name)
            || (!self.param_mode && self.stmt_lambda_names.contains(name))
    }

    // The first of `names` not yet used in this method, now taken.
    // Names the Rust file doesn't use come first, so a later Rust `i` keeps its own name.
    pub fn claim_first(&mut self, names: &[&str]) -> Option<String> {
        let name = names
            .iter()
            .find(|n| !self.name_taken(n) && !self.file_idents.contains(**n))
            .or_else(|| names.iter().find(|n| !self.name_taken(n)))?;
        self.method_names.insert(name.to_string());
        Some(name.to_string())
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    // Declares a Rust binding; returns its Java name (renamed on shadowing / keyword clash).
    pub fn declare(&mut self, rust: &str) -> String {
        let base = escape_keyword(rust);
        let mut name = base.clone();
        let mut n = 2;
        // A shadowed `x` becomes `x2`, skipping names the Rust file uses itself.
        let sep = if base.ends_with(|c: char| c.is_ascii_digit()) {
            "_"
        } else {
            ""
        };
        while self.name_taken(&name) {
            name = format!("{base}{sep}{n}");
            n += 1;
            if self.file_idents.contains(&name) {
                name = base.clone();
            }
        }
        self.method_names.insert(name.clone());
        if let Some(s) = self.scopes.last_mut() {
            s.insert(rust.to_string(), name.clone());
        }
        name
    }

    // Makes `rust` stand for existing Java text (e.g. `STDIN`) in the current scope.
    pub fn declare_as(&mut self, rust: &str, java: &str) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(rust.to_string(), java.to_string());
        }
    }

    pub fn lookup(&self, rust: &str) -> String {
        for a in self.aliases.iter().rev() {
            if let Some(n) = a.get(rust) {
                return n.clone();
            }
        }
        for s in self.scopes.iter().rev() {
            if let Some(n) = s.get(rust) {
                return n.clone();
            }
        }
        escape_keyword(rust)
    }

    // Fresh temporary name, e.g. `tmp_1`.
    // A generated local: a plain word (`item`, `text`, then `text2`, ...) that no Rust identifier in
    // the file, no hard-coded name in generated code and no Java keyword uses.
    pub fn fresh(&mut self, base: &str) -> String {
        let base = natural_name(base);
        let mut n = 1;
        loop {
            let name = match n {
                1 => base.to_string(),
                _ if base.ends_with(|c: char| c.is_ascii_digit()) => format!("{base}_{n}"),
                _ => format!("{base}{n}"),
            };
            n += 1;
            if !self.name_taken(&name)
                && !self.file_idents.contains(&name)
                && !RESERVED_NAMES.contains(&name.as_str())
                && escape_keyword(&name) == name
            {
                self.tmp_counter += 1;
                self.method_names.insert(name.clone());
                return name;
            }
        }
    }

    // Binds a Rust name to an existing Java name (a `move` closure's copy of a cell).
    pub fn bind_name(&mut self, rust: &str, java: &str) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(rust.to_string(), java.to_string());
        }
    }

    // Aliases for the duration of one condition (e.g. a pattern binding's value expression).
    pub fn push_alias(&mut self) {
        self.aliases.push(HashMap::new());
    }

    pub fn pop_alias(&mut self) {
        self.aliases.pop();
    }

    pub fn bind_alias(&mut self, rust: &str, java: String) {
        if let Some(a) = self.aliases.last_mut() {
            a.insert(rust.to_string(), java);
        }
    }

    pub fn import(&mut self, imp: &str) {
        self.imports.insert(imp.to_string());
    }
}

// `r.read_line(&mut s)` / `read_to_string(&mut s)` are lowered in place as `s += ...`, so the
// borrow never leaves the statement and `s` can stay a plain local.
fn is_inline_read_arg(r: &ast::RefExpr) -> bool {
    r.syntax()
        .parent()
        .filter(|p| ast::ArgList::can_cast(p.kind()))
        .and_then(|p| p.parent())
        .and_then(ast::MethodCallExpr::cast)
        .and_then(|m| m.name_ref())
        .is_some_and(|n| matches!(&*n.text(), "read_line" | "read_to_string"))
}

// Names written literally into generated Java (`Ok(var v)`, `equals(Object o)`, `l -> l + "\n"`, ...).
const RESERVED_NAMES: &[&str] = &[
    "v", "e", "o", "other", "l", "x0", "args", "ARGS", "STDIN", "this", "it", "_",
];

// Generator-internal base names -> the word a person would write.
fn natural_name(base: &str) -> &str {
    match base {
        "fmt_some" | "some_value" | "ok_value" | "try_value" | "vec_value" | "val"
        | "dbg_value" => "value",
        "fmt_item" | "vec_item" => "item",
        "fmt_entry" => "entry",
        "fmt_char" => "ch",
        "fmt_text" => "text",
        "fmt_ok" => "ok",
        "fmt_err" => "err",
        "fmt_pad" => "pad",
        "fmt_count" => "count",
        "fmt_value" => "value",
        "fmt_arg" => "arg",
        "fmt_f" => "f",
        "err_value" => "err",
        "vec_index" => "index",
        "res" => "result",
        "tmp" => "tmp",
        "unused" | "ignored" => "unused",
        other => other,
    }
}

// A readable name for a temporary holding `e`, taken from the callee, method or field it reads
// (`depth(a)` -> `depth`, `p.speed()` -> `speed`, `words[i]` -> `word`). None when nothing fits.
pub fn name_hint(e: &ast::Expr) -> Option<String> {
    let raw = match strip_parens(e) {
        ast::Expr::MethodCallExpr(m) => {
            let name = m.name_ref()?.text().to_string();
            if matches!(
                name.as_str(),
                "unwrap"
                    | "expect"
                    | "clone"
                    | "cloned"
                    | "copied"
                    | "to_owned"
                    | "as_ref"
                    | "as_mut"
                    | "as_str"
                    | "as_slice"
                    | "borrow"
                    | "borrow_mut"
                    | "unwrap_or"
                    | "unwrap_or_default"
                    | "unwrap_or_else"
                    | "into"
                    | "iter"
                    | "into_iter"
                    | "iter_mut"
                    | "trim"
                    | "ok"
                    | "lock"
                    | "deref"
                    | "to_string"
                    | "rev"
                    | "collect"
                    | "ok_or"
                    | "ok_or_else"
                    | "map_err"
                    | "context"
                    | "with_context"
            ) {
                return m.receiver().and_then(|r| name_hint(&r));
            }
            // Verbs name what they hand back (`stack.pop()` -> `popped`, `words.get(i)` -> `word`).
            match name.as_str() {
                "pop" | "pop_front" | "pop_back" | "pop_first" | "pop_last" => "popped".into(),
                "next" | "next_back" => "next".into(),
                "peek" | "peek_mut" => "peeked".into(),
                "find" | "find_map" => "found".into(),
                "remove" | "swap_remove" => "removed".into(),
                "take" | "replace" => "taken".into(),
                "get" | "get_mut" => {
                    let base = match m.receiver()? {
                        ast::Expr::PathExpr(p) => {
                            p.path()?.segment()?.name_ref()?.text().to_string()
                        }
                        ast::Expr::FieldExpr(f) => f.name_ref()?.text().to_string(),
                        _ => return None,
                    };
                    singular(&base)?
                }
                _ => name,
            }
        }
        ast::Expr::CallExpr(c) => {
            let ast::Expr::PathExpr(p) = c.expr()? else {
                return None;
            };
            let path = p.path()?;
            let last = path.segment()?.name_ref()?.text().to_string();
            match last.as_str() {
                "Some" | "Ok" | "Err" | "Box" | "Rc" | "Arc" | "RefCell" | "Cell" => {
                    use ast::HasArgList;
                    return c.arg_list()?.args().next().and_then(|a| name_hint(&a));
                }
                "new" | "default" | "from" | "with_capacity" | "from_str" => {
                    let owner = path.qualifier()?.segment()?.name_ref()?.text().to_string();
                    snake(&owner)
                }
                l if l.starts_with(char::is_uppercase) => snake(l),
                _ => last,
            }
        }
        ast::Expr::FieldExpr(f) => {
            let name = f.name_ref()?.text().to_string();
            if name.starts_with(|c: char| c.is_ascii_digit()) {
                return f.expr().and_then(|b| name_hint(&b));
            }
            name
        }
        ast::Expr::IndexExpr(ix) => {
            let base = match ix.base()? {
                ast::Expr::PathExpr(p) => p.path()?.segment()?.name_ref()?.text().to_string(),
                b => name_hint(&b)?,
            };
            singular(&base)?
        }
        ast::Expr::TryExpr(t) => return name_hint(&t.expr()?),
        ast::Expr::RefExpr(r) => return name_hint(&r.expr()?),
        ast::Expr::PrefixExpr(p) if p.op_kind() == Some(ast::UnaryOp::Deref) => {
            return name_hint(&p.expr()?);
        }
        ast::Expr::MacroExpr(m) => match m
            .macro_call()?
            .path()?
            .segment()?
            .name_ref()?
            .text()
            .to_string()
            .as_str()
        {
            "format" => "text".into(),
            "vec" => "items".into(),
            _ => return None,
        },
        _ => return None,
    };
    clean_hint(&raw)
}

// A name for a value from where it goes: `tokens.push(<e>)` -> `token`, `P { x: <e> }` -> `x`,
// `return <e>` -> `result`. Used when the value itself (a `match`, an `if`) says nothing.
pub fn context_hint(e: &ast::Expr) -> Option<String> {
    let parent = e.syntax().parent()?;
    if let Some(f) = ast::RecordExprField::cast(parent.clone()) {
        return clean_hint(&f.field_name()?.text());
    }
    if ast::ReturnExpr::can_cast(parent.kind()) {
        return Some("result".into());
    }
    let call = ast::ArgList::cast(parent)?.syntax().parent()?;
    let m = ast::MethodCallExpr::cast(call)?;
    let method = m.name_ref()?.text().to_string();
    if !matches!(
        method.as_str(),
        "push" | "push_back" | "push_front" | "insert" | "add"
    ) {
        return None;
    }
    let base = match m.receiver()? {
        ast::Expr::PathExpr(p) => p.path()?.segment()?.name_ref()?.text().to_string(),
        ast::Expr::FieldExpr(f) => f.name_ref()?.text().to_string(),
        _ => return None,
    };
    singular(&base)
}

// The hint for a Java expression (when the Rust one is gone): the user method or field it reads.
pub fn expr_hint(e: &jir::Expr) -> Option<String> {
    match e {
        jir::Expr::Field(_, f) => clean_hint(f),
        jir::Expr::Call { recv, name, .. } => {
            let own = match recv.as_deref() {
                None | Some(jir::Expr::Name(_)) => true,
                Some(jir::Expr::Raw(r)) => r == "this",
                _ => false,
            };
            // Java library names (`size`, `getKey`, `valueOf`) say little about the value.
            if own
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
                && !matches!(
                    name.as_str(),
                    "get" | "size" | "length" | "of" | "apply" | "test" | "next"
                )
            {
                clean_hint(name)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn clean_hint(raw: &str) -> Option<String> {
    let mut s = raw.trim_end_matches("_mut");
    for prefix in ["get_", "to_", "as_", "into_"] {
        if let Some(rest) = s.strip_prefix(prefix)
            && !rest.is_empty()
        {
            s = rest;
        }
    }
    let ok = !s.is_empty()
        && s.len() <= 24
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !matches!(s, "self" | "new" | "main" | "fmt" | "call" | "default");
    ok.then(|| s.to_string())
}

// `HashMap` -> `hash_map`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// `words` -> `word`; None when the name isn't a plain plural.
fn singular(name: &str) -> Option<String> {
    let s = name.strip_suffix('s')?;
    (s.len() >= 2 && !s.ends_with('s')).then(|| s.to_string())
}

// The name for a destructured parameter (`|(i, l)|`, `|Point { x, y }|`).
pub fn pat_hint(p: &ast::Pat) -> &'static str {
    match p {
        ast::Pat::TuplePat(t) if t.fields().count() == 2 => "pair",
        ast::Pat::TuplePat(_) => "tuple",
        ast::Pat::RefPat(r) => r.pat().map_or("arg", |p| pat_hint(&p)),
        _ => "arg",
    }
}

pub fn strip_parens(e: &ast::Expr) -> ast::Expr {
    match e {
        ast::Expr::ParenExpr(p) => p
            .expr()
            .map(|i| strip_parens(&i))
            .unwrap_or_else(|| e.clone()),
        _ => e.clone(),
    }
}

// Types whose Java representation is immutable, so `&mut` needs a cell.
// Types a `&mut` local/param must box in a one-element array: Java can't write through them.
// Values Java can't change in place, so a `&mut` to one needs a one-element array: numbers,
// strings, `Optional` and tuples (`Map.Entry` / records).
pub fn is_mut_ref_cell_ty(ty: &Ty) -> bool {
    is_cell_value_ty(ty)
        || matches!(ty.strip_refs(), Ty::Option(_))
        || matches!(ty.strip_refs(), Ty::Tuple(ts) if ts.len() >= 2)
}

pub fn is_cell_value_ty(ty: &Ty) -> bool {
    matches!(
        ty.strip_refs(),
        Ty::Bool | Ty::Char | Ty::Int(_) | Ty::F32 | Ty::F64 | Ty::Str
    )
}

const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "var",
    "record",
    "yield",
    "sealed",
    "permits",
    "String",
    "Object",
    "Integer",
    "Long",
    "Double",
    "List",
    "Map",
    "Optional",
];

pub fn escape_keyword(s: &str) -> String {
    let s = s.strip_prefix("r#").unwrap_or(s);
    if JAVA_KEYWORDS.contains(&s) {
        format!("{s}_")
    } else {
        s.to_string()
    }
}
