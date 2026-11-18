// Rust syntax tree (+ semantics) -> JIR. One submodule per construct family.

pub mod calls;
pub mod expr;
pub mod fmt;
pub mod items;
pub mod iter;
pub mod macros;
pub mod pat;
pub mod std_map;
pub mod stmt;
pub mod traits;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use ra_ap_hir::{Module, Semantics};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, ast};

use crate::frontend::{self, Unit};
use crate::jir::{Expr, JavaFile};
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
    // Set when the program uses `Result`, so `Result.java` gets written.
    pub needs_result: bool,
    // Enclosing loops/labeled blocks: (Java label, variable receiving `break value`).
    pub loops: Vec<LoopFrame>,
    // Number of enclosing Java switch statements (affects unlabeled `break`).
    pub switch_depth: u32,
    // Locals stored in one-element arrays (`int[] n = {0}`): `&mut` targets and closure-mutated vars.
    pub cells: HashSet<ra_ap_hir::Local<'db>>,
    // Locals that *are* a one-element array: `Cell<i32>`/`RefCell<i32>` handles (`int[] c = {0}`).
    pub cell_handles: HashSet<ra_ap_hir::Local<'db>>,
    // >0 while lowering a closure body: statements hoisted to `out` would run only once, outside it.
    pub lambda_depth: u32,
    // Iterator locals pulled with `next()`/`peek()`: Java `ListIterator` + the backing list's name.
    pub pulled_iters: HashMap<ra_ap_hir::Local<'db>, String>,
    // `for x in v.iter_mut()` over immutable values: `x` stands for `list.get(idx)`.
    pub elem_aliases: HashMap<ra_ap_hir::Local<'db>, (String, String, ElemAliasKind)>,
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
    // Every identifier in the Rust file: generated names never take one of these.
    file_idents: HashSet<String>,
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
}

#[derive(Clone, Debug)]
pub struct LoopFrame {
    pub rust_label: Option<String>,
    pub java_label: Option<String>,
    pub value_var: Option<String>,
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
    pub needs_result: bool,
}

pub fn lower_unit(sema: &Semantics<'_, RootDatabase>, unit: &Unit) -> Lowered {
    let file = sema.parse(unit.file_id);
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
        scopes: Vec::new(),
        aliases: Vec::new(),
        method_names: HashSet::new(),
        tmp_counter: 0,
        ret_ty: None,
        file_trailer: Vec::new(),
        needs_result: false,
        loops: Vec::new(),
        switch_depth: 0,
        cells: HashSet::new(),
        cell_handles: HashSet::new(),
        lambda_depth: 0,
        pulled_iters: HashMap::new(),
        elem_aliases: HashMap::new(),
        local_traits: HashMap::new(),
        macro_methods: HashMap::new(),
        macro_method_defs: Vec::new(),
        macro_lines: None,
        uses_args: false,
        stdin_lines: false,
        read_only_literal: false,
        file_idents: file
            .syntax()
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == SyntaxKind::IDENT)
            .map(|t| t.text().to_string())
            .collect(),
    };
    cx.stdin_lines = !std_map::io::has_byte_reads(&cx, file.syntax());
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
    Lowered {
        file: jf,
        comments,
        trailer: cx.file_trailer.join(" "),
        last_line,
        diags: cx.diags,
        needs_result: cx.needs_result,
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

    // Records an unsupported construct and returns a placeholder so lowering can continue.
    pub fn unsupported(&mut self, node: &SyntaxNode, what: &str) -> Expr {
        let line = self.line(node);
        let col = self.col(node);
        self.diags.push(Diag {
            line,
            col,
            msg: format!("unsupported: {what}"),
        });
        Expr::raw(format!("/* unsupported: {what} */ null"))
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
                .is_some_and(|r| r.mut_token().is_some() && !is_inline_read_arg(&r));
            let in_closure = pe
                .syntax()
                .ancestors()
                .take_while(|n| *n != scope)
                .filter_map(ast::ClosureExpr::cast)
                .any(|c| {
                    !c.syntax()
                        .text_range()
                        .contains_range(ip.syntax().text_range())
                });
            if borrowed_mut || (is_mut && in_closure) {
                return true;
            }
        }
        false
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
        while self.method_names.contains(&name) {
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
            if !self.method_names.contains(&name)
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
pub fn is_mut_ref_cell_ty(ty: &Ty) -> bool {
    is_cell_value_ty(ty) || matches!(ty.strip_refs(), Ty::Option(_))
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
