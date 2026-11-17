// Items: module class, functions, consts, structs, enums, impls, traits.

use ra_ap_hir::{Function, Impl, ModuleDef};
use ra_ap_syntax::{AstNode, ast, ast::HasModuleItem, ast::HasName};

use super::stmt::{self, Dest};
use super::{Cx, escape_keyword, std_map};
use crate::frontend::module_class_name;
use crate::jir::{Class, Expr, Member, Method, Stmt};
use crate::ty::Ty;

pub fn lower_source_file(cx: &mut Cx, file: &ast::SourceFile, is_root: bool) -> Class {
    let mut members = Vec::new();
    for item in file.items() {
        lower_item(cx, &item, &mut members);
    }
    if is_root {
        cx.file_trailer.push(entry_point(cx, file));
    }
    Class {
        line: 1,
        header: format!("public final class {}", cx.class_name),
        members,
        close_line: 1,
        trailer: Vec::new(),
    }
}

fn entry_point(cx: &Cx, file: &ast::SourceFile) -> String {
    let main_returns_result = file.items().any(|i| match i {
        ast::Item::Fn(f) => f.name().is_some_and(|n| n.text() == "main") && f.ret_type().is_some(),
        _ => false,
    });
    let call = if main_returns_result {
        "if (main() instanceof Err<?, ?>(var e)) { System.err.println(\"Error: \" + e); System.exit(1); }"
    } else {
        "main();"
    };
    let _ = cx;
    format!(
        "static String[] ARGS; public static void main(String[] args) {{ ARGS = args; try {{ {call} }} catch (RuntimeException e) {{ System.out.flush(); System.err.println(\"thread 'main' panicked:\\n\" + e.getMessage()); System.exit(101); }} }}"
    )
}

pub fn lower_item(cx: &mut Cx, item: &ast::Item, members: &mut Vec<Member>) {
    match item {
        ast::Item::Fn(f) => members.push(Member::Method(lower_fn(cx, f, "static "))),
        ast::Item::Use(_) | ast::Item::ExternCrate(_) => {}
        ast::Item::Const(c) => {
            if let Some(m) = lower_const(cx, c.name(), c.ty(), c.body(), c.syntax()) {
                members.push(m);
            }
        }
        ast::Item::Static(s) => {
            if s.mut_token().is_some() {
                cx.unsupported(s.syntax(), "static mut");
                return;
            }
            if let Some(m) = lower_const(cx, s.name(), s.ty(), s.body(), s.syntax()) {
                members.push(m);
            }
        }
        ast::Item::Module(m) if m.item_list().is_none() => {} // `mod foo;` is its own file/class.
        ast::Item::MacroRules(_) => {}
        _ => {
            cx.unsupported(item.syntax(), "item kind (struct/enum/impl/trait/mod: TODO)");
        }
    }
}

fn lower_const(
    cx: &mut Cx,
    name: Option<ast::Name>,
    ty: Option<ast::Type>,
    body: Option<ast::Expr>,
    node: &ra_ap_syntax::SyntaxNode,
) -> Option<Member> {
    let name = name?.text().to_string();
    let ty = ty.map(|t| cx.type_ref(&t)).unwrap_or(Ty::Other("?".into()));
    let body = body?;
    cx.enter_method();
    let mut pre = Vec::new();
    let v = super::expr::lower_expr_as(cx, &body, &ty, &mut pre);
    if !pre.is_empty() {
        cx.unsupported(node, "const initializer with statements");
    }
    Some(Member::Field {
        line: cx.line(node),
        text: format!("static final {} {} = {};", ty.java(), escape_keyword(&name), crate::emit::render_expr(&v)),
    })
}

// `modifiers` is e.g. "static " or "public " (with trailing space).
pub fn lower_fn(cx: &mut Cx, f: &ast::Fn, modifiers: &str) -> Method {
    let line = cx.line(f.syntax());
    cx.enter_method();
    let name = f.name().map(|n| n.text().to_string()).unwrap_or_default();
    let def = cx.sema.to_fn_def(f);
    let ret = match def {
        Some(d) => cx.convert(&d.ret_type(cx.db)),
        None => Ty::Unit,
    };
    let mut params = Vec::new();
    let mut prelude = Vec::new();
    if let Some(pl) = f.param_list() {
        for p in pl.params() {
            let ty = p.ty().map(|t| cx.type_ref(&t)).unwrap_or(Ty::Other("?".into()));
            let jty = param_type(cx, &ty);
            match p.pat() {
                Some(ast::Pat::IdentPat(ip)) if ip.pat().is_none() => {
                    let n = ip.name().map(|n| n.text().to_string()).unwrap_or_else(|| "_".into());
                    params.push(format!("{jty} {}", cx.declare(&n)));
                    if is_cell_param(&ty) {
                        cx.mark_cell(&ip);
                    }
                }
                Some(other) => {
                    let tmp = cx.fresh("arg");
                    params.push(format!("{jty} {tmp}"));
                    super::pat::bind_pattern(cx, &other, Expr::name(tmp), &mut prelude);
                }
                None => {}
            }
        }
    }
    let generics = generic_params(cx, f);
    let ret_java = if ret.is_unit() { "void".to_string() } else { ret.java() };
    let header = format!("{modifiers}{generics}{ret_java} {}({})", escape_keyword(&name), params.join(", "));
    cx.ret_ty = Some(ret);
    let body = f.body().map(|b| {
        let mut block = stmt::lower_block(cx, &b, &Dest::Return);
        if !prelude.is_empty() {
            prelude.append(&mut block.stmts);
            block.stmts = prelude;
        }
        block
    });
    cx.ret_ty = None;
    Method { line, header, body }
}

pub fn param_type(cx: &mut Cx, ty: &Ty) -> String {
    let _ = cx;
    if is_cell_param(ty) {
        return format!("{}[]", ty.strip_refs().java());
    }
    stmt::decl_type_explicit(ty)
}

// `&mut i32` / `&mut String` parameters are passed as one-element arrays.
fn is_cell_param(ty: &Ty) -> bool {
    matches!(ty, Ty::Ref { mutable: true, .. }) && super::is_cell_value_ty(ty)
}

// `<T extends Comparable<T>> ` for generic fns (empty when not generic).
pub fn generic_params(cx: &mut Cx, f: &ast::Fn) -> String {
    use ast::HasGenericParams;
    let Some(gpl) = f.generic_param_list() else { return String::new() };
    let names: Vec<String> = gpl
        .type_or_const_params()
        .filter_map(|p| match p {
            ast::TypeOrConstParam::Type(t) => t.name().map(|n| n.text().to_string()),
            ast::TypeOrConstParam::Const(_) => None,
        })
        .collect();
    let _ = cx;
    if names.is_empty() { String::new() } else { format!("<{}> ", names.join(", ")) }
}

pub fn lower_local_item(cx: &mut Cx, item: &ast::Item, out: &mut Vec<Stmt>) {
    let _ = out;
    match item {
        ast::Item::Use(_) => {}
        _ => {
            cx.unsupported(item.syntax(), "item inside a function body");
        }
    }
}

pub fn lower_record_expr(cx: &mut Cx, r: &ast::RecordExpr, out: &mut Vec<Stmt>) -> Expr {
    let _ = out;
    cx.unsupported(r.syntax(), "struct literal")
}

// Path in value position resolving to an item (const, static, fn value, unit variant/struct).
pub fn lower_def_path(cx: &mut Cx, path: &ast::Path, def: ModuleDef) -> Expr {
    let db = cx.db;
    let local = def.module(db).is_some_and(|m| m.krate(db).origin(db).is_local());
    if !local {
        return std_map::lower_std_path(cx, path, def);
    }
    let qualify = |cx: &Cx, name: String| -> Expr {
        match def.module(cx.db) {
            Some(m) if m != cx.module => Expr::raw(format!("{}.{name}", module_class_name(cx.db, m))),
            _ => Expr::raw(name),
        }
    };
    match def {
        ModuleDef::Const(c) => {
            let n = c.name(db).map(|n| n.as_str().to_string()).unwrap_or_default();
            qualify(cx, escape_keyword(&n))
        }
        ModuleDef::Static(s) => qualify(cx, escape_keyword(s.name(db).as_str())),
        ModuleDef::Function(f) => {
            let owner = match def.module(db) {
                Some(m) if m != cx.module => module_class_name(db, m),
                _ => cx.class_name.clone(),
            };
            Expr::raw(format!("{owner}::{}", escape_keyword(f.name(db).as_str())))
        }
        _ => cx.unsupported(path.syntax(), "path to item (variant/struct: TODO)"),
    }
}

// Tuple-struct / tuple-variant constructor calls `Foo(a, b)`.
pub fn lower_ctor_call(cx: &mut Cx, c: &ast::CallExpr, path: &ast::Path, args: Vec<Expr>, out: &mut Vec<Stmt>) -> Expr {
    let _ = out;
    if let Some(e) = std_map::lower_std_ctor(cx, c, path, &args) {
        return e;
    }
    cx.unsupported(c.syntax(), "tuple struct / variant constructor")
}

// Java name of an impl's self type, as seen from the current file.
pub fn impl_owner_java(cx: &Cx, imp: Impl) -> String {
    let ty = imp.self_ty(cx.db);
    match cx.convert(&ty) {
        Ty::Adt { java, .. } => java,
        other => other.boxed(),
    }
}

// Method calls resolving to `impl LocalTrait for ForeignType`: dispatched statically.
pub fn lower_foreign_impl_call(cx: &mut Cx, m: &ast::MethodCallExpr, f: Function, out: &mut Vec<Stmt>) -> Option<Expr> {
    let _ = (cx, m, f, out);
    None
}

// Whether `e` names a `&mut` cell (one-element array).
pub fn is_cell(cx: &Cx, e: &ast::Expr) -> bool {
    let _ = (cx, e);
    false
}

// ---- Representation contract shared by items.rs (declarations) and pat.rs (patterns) --------
//   enum with only unit variants  -> Java `enum E { A, B }`; value `E.A`; switch label `A`
//   any other enum                -> `sealed interface E` + one nested `record` per variant;
//                                    value `new E.V(..)`; pattern `E.V(var a, var b)`
//   record/tuple fields           -> named fields keep their name; tuple fields are `_0, _1, ...`
//   struct                        -> `static final class S` with the same field naming

pub fn enum_is_plain(cx: &Cx, e: ra_ap_hir::Enum) -> bool {
    e.variants(cx.db).iter().all(|v| v.kind(cx.db) == ra_ap_hir::StructKind::Unit)
}

// Java type name of a local ADT as seen from the current file (`Shape` or `Geometry.Shape`).
pub fn adt_java_name(cx: &Cx, adt: ra_ap_hir::Adt) -> String {
    let name = escape_keyword(adt.name(cx.db).as_str());
    let m = adt.module(cx.db);
    if m == cx.module { name } else { format!("{}.{name}", module_class_name(cx.db, m)) }
}

// `Shape.Circle` for any variant of a local enum.
pub fn variant_java_name(cx: &Cx, v: ra_ap_hir::EnumVariant) -> String {
    let e = adt_java_name(cx, ra_ap_hir::Adt::Enum(v.parent_enum(cx.db)));
    format!("{e}.{}", escape_keyword(v.name(cx.db).as_str()))
}

// Java field names of a struct/variant, in declaration order.
pub fn java_field_names(cx: &Cx, fields: &[ra_ap_hir::Field]) -> Vec<String> {
    fields
        .iter()
        .map(|f| {
            let n = f.name(cx.db);
            let s = n.as_str();
            if s.chars().all(|c| c.is_ascii_digit()) { format!("_{s}") } else { escape_keyword(s) }
        })
        .collect()
}

// Records that `TupleN` is needed; emitted once per file on the last line.
pub fn need_tuple(cx: &mut Cx, n: usize) {
    let params: Vec<String> = (0..n).map(|i| format!("T{i}")).collect();
    let fields: Vec<String> = (0..n).map(|i| format!("T{i} _{i}")).collect();
    let decl = format!("record Tuple{n}<{}>({}) {{}}", params.join(", "), fields.join(", "));
    if !cx.file_trailer.contains(&decl) {
        cx.file_trailer.insert(0, decl);
    }
}
