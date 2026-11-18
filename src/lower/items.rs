// Items: module class, functions, consts, structs, enums, impls, traits.

use ra_ap_hir::{
    Adt, AsAssocItem, AssocItemContainer, Function, HasSource, Impl, ModuleDef, StructKind, Trait,
    Variant,
};
use ra_ap_syntax::{
    AstNode, SyntaxNode, ast, ast::HasArgList, ast::HasAttrs, ast::HasGenericParams,
    ast::HasModuleItem, ast::HasName, ast::HasTypeBounds, ast::HasVisibility,
};

use super::stmt::{self, Dest};
use super::{Cx, escape_keyword, expr, std_map, traits};
use crate::frontend::{module_class_from, pascal_case, trait_java_name};
use crate::jir::{Class, Expr, Member, Method, Stmt, StmtKind};
use crate::ty::{Int, Ty};

// A field of a struct or enum variant, as Java sees it.
#[derive(Clone)]
struct FieldInfo {
    line: u32,
    name: String,
    ty: Ty,
    vis: String,
    plain_enum: bool,
    // Safe to share (not copy) when cloning: Java records are immutable, containers are not.
    shallow_clone: bool,
}

// The Java type an impl block's items belong to.
struct Target {
    java: String,
    // Data enums are emitted as an interface, so their instance methods need `default`.
    interface: bool,
    // Type parameters of the type; static methods must redeclare them in Java.
    generics: Vec<String>,
}

pub fn lower_source_file(cx: &mut Cx, file: &ast::SourceFile, is_root: bool) -> Class {
    let items: Vec<ast::Item> = file.items().filter(|i| !is_test_only(i)).collect();
    traits::collect(cx, &items);
    let mut members = Vec::new();
    lower_items(cx, &items, &mut members);
    // Macro methods sit where their `macro_rules!` was written.
    for m in std::mem::take(&mut cx.macro_method_defs) {
        let at = members
            .iter()
            .position(|x| member_line(x) > m.line)
            .unwrap_or(members.len());
        members.insert(at, Member::Method(m));
    }
    if is_root {
        let entry = entry_point(cx, file);
        cx.file_trailer.push(entry);
    }
    Class {
        line: 1,
        header: format!("public final class {}", cx.class_name),
        members,
        close_line: 1,
        trailer: Vec::new(),
    }
}

fn entry_point(cx: &mut Cx, file: &ast::SourceFile) -> String {
    let result_err = file.items().find_map(|i| match i {
        ast::Item::Fn(f) if f.name().is_some_and(|n| n.text() == "main") => f
            .ret_type()
            .and_then(|t| t.ty())
            .and_then(|t| cx.sema.resolve_type(&t))
            .map(|t| cx.convert(&t))
            .and_then(|t| match t.strip_refs() {
                Ty::Result(_, e) => Some((**e).clone()),
                _ => None,
            }),
        _ => None,
    });
    let call = match result_err {
        Some(err) => {
            // The pattern variable is Object; user error types need their Java type for `debug()`.
            let value = if err.is_nameable() {
                Expr::Cast(err.java(), Box::new(Expr::name("e")))
            } else {
                Expr::name("e")
            };
            let debug = crate::lower::fmt::debug(cx, value, &err);
            format!(
                "if (main() instanceof Err<?, ?>(var e)) {{ System.err.println(\"Error: \" + ({})); System.exit(1); }}",
                crate::emit::render_expr(&debug)
            )
        }
        None => "main();".into(),
    };
    let (field, keep) = if cx.uses_args {
        ("static String[] ARGS; ", "ARGS = args; ")
    } else {
        ("", "")
    };
    format!(
        "{field}public static void main(String[] args) {{ {keep}try {{ {call} }} catch (RuntimeException e) {{ System.out.flush(); System.err.println(\"thread 'main' panicked:\\n\" + e.getMessage()); System.exit(101); }} }}"
    )
}

// Lowers one item list (a file or an inline `mod` body) into Java members.
fn lower_items(cx: &mut Cx, items: &[ast::Item], members: &mut Vec<Member>) {
    let adts: Vec<Option<Adt>> = items.iter().map(|i| adt_of_item(cx, i)).collect();
    let mut claimed = vec![false; items.len()];
    // Foreign trait impls share one `<Trait>Impls` class per trait.
    let mut impl_classes: Vec<(String, usize)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if claimed[i] {
            continue;
        }
        match item {
            ast::Item::Struct(s) => {
                let impls = claim_impls(cx, items, adts[i], &mut claimed);
                members.push(Member::Class(lower_struct(cx, s, adts[i], impls)));
            }
            ast::Item::Enum(e) => {
                let impls = claim_impls(cx, items, adts[i], &mut claimed);
                members.push(Member::Class(lower_enum(cx, e, adts[i], impls)));
            }
            ast::Item::Impl(imp) => {
                let adt = impl_self_adt(cx, imp);
                let defined_here = adt.is_some_and(|a| adts.iter().flatten().any(|x| *x == a));
                if adt.is_some() && !defined_here {
                    cx.error(
                        imp.syntax(),
                        "impl for a type defined in another file is not supported",
                    );
                    continue;
                }
                if adt.is_some() {
                    continue;
                }
                let Some(class) = lower_foreign_impl(cx, imp) else {
                    continue;
                };
                match impl_classes
                    .iter()
                    .find(|(n, _)| *n == class_name_of(&class.header))
                {
                    Some((_, idx)) => {
                        let Member::Class(existing) = &mut members[*idx] else {
                            continue;
                        };
                        existing.members.extend(class.members);
                        existing.close_line = existing.close_line.max(class.close_line);
                    }
                    None => {
                        impl_classes.push((class_name_of(&class.header), members.len()));
                        members.push(Member::Class(class));
                    }
                }
            }
            ast::Item::Trait(t) => {
                if let Some(m) = lower_trait(cx, t) {
                    members.push(m);
                }
            }
            ast::Item::Module(m) if m.item_list().is_some() => {
                members.push(lower_inline_module(cx, m));
            }
            ast::Item::Fn(f) => {
                let vis = if f.visibility().is_some() {
                    "public "
                } else {
                    ""
                };
                members.push(Member::Method(lower_fn(cx, f, &format!("{vis}static "))));
            }
            ast::Item::Const(c) => {
                if let Some(m) = lower_const(cx, c.name(), c.ty(), c.body(), c.syntax()) {
                    members.push(m);
                }
            }
            ast::Item::Static(s) => {
                if s.mut_token().is_some() {
                    cx.unsupported(s.syntax(), "static mut");
                    continue;
                }
                if let Some(m) = lower_const(cx, s.name(), s.ty(), s.body(), s.syntax()) {
                    members.push(m);
                }
            }
            ast::Item::MacroCall(mc) if super::macros::macro_name(mc) == "thread_local" => {
                lower_thread_local(cx, mc, members);
            }
            ast::Item::Use(_) | ast::Item::ExternCrate(_) | ast::Item::MacroRules(_) => {}
            ast::Item::Module(_) => {}
            ast::Item::TypeAlias(_) => {}
            _ => {
                cx.unsupported(item.syntax(), "item kind");
            }
        }
    }
}

// A Rust method name as a Java method: keywords get `_`, and so do names Java reserves as final
// (`Enum.name()`/`ordinal()` on fieldless enums, which are Java enums; `Object.getClass()`/`wait()`).
pub fn method_java_name(cx: &Cx, func: ra_ap_hir::Function) -> String {
    let name = func.name(cx.db).as_str().to_string();
    const ENUM_FINAL: &[&str] = &[
        "name",
        "ordinal",
        "compareTo",
        "getDeclaringClass",
        "describeConstable",
        "values",
        "valueOf",
    ];
    const OBJECT_FINAL: &[&str] = &["getClass", "wait", "notify", "notifyAll"];
    let on_java_enum = func
        .as_assoc_item(cx.db)
        .and_then(|a| match a.container(cx.db) {
            ra_ap_hir::AssocItemContainer::Impl(i) => i.self_ty(cx.db).as_adt(),
            _ => None,
        })
        .is_some_and(|adt| matches!(adt, Adt::Enum(e) if enum_is_plain(cx, e)));
    if (on_java_enum && ENUM_FINAL.contains(&name.as_str()))
        || OBJECT_FINAL.contains(&name.as_str())
    {
        return format!("{name}_");
    }
    escape_keyword(&name)
}

fn class_name_of(header: &str) -> String {
    header.rsplit(' ').next().unwrap_or(header).to_string()
}

fn adt_of_item(cx: &Cx, item: &ast::Item) -> Option<Adt> {
    match item {
        ast::Item::Struct(s) => cx.sema.to_struct_def(s).map(Adt::Struct),
        ast::Item::Enum(e) => cx.sema.to_enum_def(e).map(Adt::Enum),
        _ => None,
    }
}

// The local ADT an impl block extends; std types count as foreign.
pub(crate) fn impl_self_adt(cx: &Cx, imp: &ast::Impl) -> Option<Adt> {
    let def = cx.sema.to_impl_def(imp)?;
    let adt = def.self_ty(cx.db).strip_references().as_adt()?;
    adt.module(cx.db)
        .krate(cx.db)
        .origin(cx.db)
        .is_local()
        .then_some(adt)
}

fn claim_impls<'a>(
    cx: &Cx,
    items: &'a [ast::Item],
    adt: Option<Adt>,
    claimed: &mut [bool],
) -> Vec<&'a ast::Impl> {
    let Some(adt) = adt else { return Vec::new() };
    let mut out = Vec::new();
    for (j, item) in items.iter().enumerate() {
        if claimed[j] {
            continue;
        }
        if let ast::Item::Impl(imp) = item
            && impl_self_adt(cx, imp) == Some(adt)
        {
            claimed[j] = true;
            out.push(imp);
        }
    }
    out
}

// --- structs -----------------------------------------------------------------------------

fn lower_struct(cx: &mut Cx, s: &ast::Struct, adt: Option<Adt>, impls: Vec<&ast::Impl>) -> Class {
    let name = escape_keyword(&s.name().map(|n| n.text().to_string()).unwrap_or_default());
    let line = cx.line(s.syntax());
    let vis = if s.visibility().is_some() {
        "public "
    } else {
        ""
    };
    let kind = adt
        .and_then(|a| a.as_struct())
        .map(|st| st.kind(cx.db))
        .unwrap_or(StructKind::Record);
    let fields = field_list_info(cx, s.field_list());
    let close_line = impls
        .iter()
        .map(|i| cx.end_line(i.syntax()))
        .max()
        .unwrap_or(0)
        .max(cx.end_line(s.syntax()));
    let generic_decls = generic_param_decls(cx, s.generic_param_list(), s.where_clause());
    let generics = wrap_generics(&generic_decls);
    let derives = derives_of(s);
    let mut members: Vec<Member> = fields
        .iter()
        .map(|f| Member::Field {
            line: f.line,
            text: format!("{}{} {};", f.vis, field_java(&f.ty), f.name),
        })
        .collect();
    let mut implements = Vec::new();
    let target = Target {
        java: adt_java_name_opt(cx, adt, &name),
        interface: false,
        generics: generic_decls.clone(),
    };
    if !impls.is_empty() {
        let (impl_members, traits) = lower_impls(cx, &impls, &target);
        members.extend(impl_members);
        implements.extend(traits);
    }
    if derives.iter().any(|d| d == "PartialOrd" || d == "Ord") {
        implements.push(format!("Comparable<{name}>"));
    }
    let mut trailer = Vec::new();
    if kind == StructKind::Unit {
        trailer.push(format!("{vis}static final {name} INSTANCE = new {name}();"));
        trailer.push(format!("private {name}() {{}}"));
    } else {
        trailer.push(canonical_ctor(&name, &fields));
    }
    trailer.extend(derive_methods(
        cx,
        s.syntax(),
        &name,
        &generic_decls,
        kind,
        &fields,
        &derives,
        true,
    ));
    let impl_clause = if implements.is_empty() {
        String::new()
    } else {
        format!(" implements {}", implements.join(", "))
    };
    Class {
        line,
        header: format!("{vis}static final class {name}{generics}{impl_clause}"),
        members,
        close_line,
        trailer,
    }
}

fn adt_java_name_opt(cx: &Cx, adt: Option<Adt>, fallback: &str) -> String {
    adt.map(|a| adt_java_name(cx, a))
        .unwrap_or_else(|| fallback.to_string())
}

// An iterator kept in a field is pulled step by step (`self.chars.next()`), so it is a
// `ListIterator` over the materialized items rather than a one-shot `Stream`.
fn field_java(ty: &Ty) -> String {
    match ty {
        Ty::Iter(t) => format!("ListIterator<{}>", t.boxed()),
        t => t.java(),
    }
}

fn canonical_ctor(name: &str, fields: &[FieldInfo]) -> String {
    let params: Vec<String> = fields
        .iter()
        .map(|f| format!("{} {}", field_java(&f.ty), f.name))
        .collect();
    let body: Vec<String> = fields
        .iter()
        .map(|f| format!("this.{0} = {0};", f.name))
        .collect();
    format!(
        "public {name}({}) {{ {} }}",
        params.join(", "),
        body.join(" ")
    )
}

// Java enum constants have no callable `clone()`; copying the reference is already a clone.
fn ty_is_plain_enum(cx: &Cx, t: &ast::Type) -> bool {
    let Some(ty) = cx.sema.resolve_type(t) else {
        return false;
    };
    let Some(Adt::Enum(e)) = ty.as_adt() else {
        return false;
    };
    e.variants(cx.db)
        .iter()
        .all(|v| v.kind(cx.db) == StructKind::Unit)
}

// Whether a clone of a value of this type may share the Java object instead of copying it.
// A data-enum variant is a Java record, so it is shareable once all its fields are.
fn clone_is_shallow(cx: &Cx, t: &ast::Type) -> bool {
    cx.sema
        .resolve_type(t)
        .is_some_and(|ty| shallow_clone_ty(cx, &ty, 0))
}

fn shallow_clone_ty(cx: &Cx, ty: &ra_ap_hir::Type<'_>, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    if ty.is_unit() || ty.is_never() || ty.is_bool() || ty.is_char() || ty.is_str() {
        return true;
    }
    // Cloning a reference copies the reference, which the Java field already holds.
    if ty.is_reference() {
        return true;
    }
    if let Some(b) = ty.as_builtin() {
        let name = b.name();
        let name = name.as_str();
        return crate::ty::Int::from_name(name).is_some() || matches!(name, "f32" | "f64");
    }
    if ty.as_slice().is_some() || ty.as_array(cx.db).is_some() {
        return false;
    }
    if ty.is_tuple() {
        return ty
            .tuple_fields(cx.db)
            .iter()
            .all(|t| shallow_clone_ty(cx, t, depth + 1));
    }
    if ty.as_type_param(cx.db).is_some() {
        return false;
    }
    let Some(adt) = ty.as_adt() else {
        return false;
    };
    if !adt.module(cx.db).krate(cx.db).origin(cx.db).is_local() {
        return match adt.name(cx.db).as_str() {
            "String" => true,
            // Cloning these shares the pointee in Rust too.
            "Rc" | "Arc" => true,
            "Option" | "Result" => ty
                .type_arguments()
                .all(|t| shallow_clone_ty(cx, &t, depth + 1)),
            _ => false,
        };
    }
    let Adt::Enum(e) = adt else {
        return false;
    };
    e.variants(cx.db).iter().all(|v| {
        v.fields(cx.db)
            .iter()
            .all(|f| shallow_clone_ty(cx, &f.ty(cx.db), depth + 1))
    })
}

fn field_list_info(cx: &mut Cx, list: Option<ast::FieldList>) -> Vec<FieldInfo> {
    let mut out = Vec::new();
    match list {
        Some(ast::FieldList::RecordFieldList(rfl)) => {
            for f in rfl.fields() {
                let name =
                    escape_keyword(&f.name().map(|n| n.text().to_string()).unwrap_or_default());
                let ty = f
                    .ty()
                    .map(|t| cx.type_ref(&t))
                    .unwrap_or(Ty::Other("?".into()));
                let vis = if f.visibility().is_some() {
                    "public ".to_string()
                } else {
                    String::new()
                };
                super::std_map::ptr::note_fn(cx, f.syntax(), &ty);
                let plain_enum = f.ty().is_some_and(|t| ty_is_plain_enum(cx, &t));
                let shallow_clone = f.ty().is_some_and(|t| clone_is_shallow(cx, &t));
                out.push(FieldInfo {
                    line: cx.line(f.syntax()),
                    name,
                    ty,
                    vis,
                    plain_enum,
                    shallow_clone,
                });
            }
        }
        Some(ast::FieldList::TupleFieldList(tfl)) => {
            for (i, f) in tfl.fields().enumerate() {
                let ty = f
                    .ty()
                    .map(|t| cx.type_ref(&t))
                    .unwrap_or(Ty::Other("?".into()));
                let vis = if f.visibility().is_some() {
                    "public ".to_string()
                } else {
                    String::new()
                };
                let plain_enum = f.ty().is_some_and(|t| ty_is_plain_enum(cx, &t));
                let shallow_clone = f.ty().is_some_and(|t| clone_is_shallow(cx, &t));
                out.push(FieldInfo {
                    line: cx.line(f.syntax()),
                    name: format!("_{i}"),
                    ty,
                    vis,
                    plain_enum,
                    shallow_clone,
                });
            }
        }
        None => {}
    }
    out
}

// --- enums -------------------------------------------------------------------------------

fn lower_enum(cx: &mut Cx, e: &ast::Enum, adt: Option<Adt>, impls: Vec<&ast::Impl>) -> Class {
    let name = escape_keyword(&e.name().map(|n| n.text().to_string()).unwrap_or_default());
    let line = cx.line(e.syntax());
    let vis = if e.visibility().is_some() {
        "public "
    } else {
        ""
    };
    let eh = adt.and_then(|a| a.as_enum());
    let plain = eh.is_some_and(|en| enum_is_plain(cx, en));
    let close_line = impls
        .iter()
        .map(|i| cx.end_line(i.syntax()))
        .max()
        .unwrap_or(0)
        .max(cx.end_line(e.syntax()));
    let generic_decls = generic_param_decls(cx, e.generic_param_list(), e.where_clause());
    let generics = wrap_generics(&generic_decls);
    let derives = derives_of(e);
    let target = Target {
        java: name.clone(),
        interface: !plain,
        generics: generic_decls.clone(),
    };
    let (impl_members, mut implements) = if impls.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        lower_impls(cx, &impls, &target)
    };
    let mut members = Vec::new();
    let mut trailer = Vec::new();
    let header;
    if plain {
        let Some(en) = eh else {
            return empty_class(line, close_line, &name);
        };
        if derives.iter().any(|d| d == "Ord" || d == "PartialOrd")
            && en.variants(cx.db).iter().any(|v| v.value(cx.db).is_some())
        {
            cx.error(
                e.syntax(),
                "derive(Ord) on an enum with explicit discriminants is not supported",
            );
        }
        let variants: Vec<(String, i128, u32)> = en
            .variants(cx.db)
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let vname = escape_keyword(v.name(cx.db).as_str());
                let value = v.eval(cx.db).unwrap_or(i as i128);
                let vline = v
                    .source(cx.db)
                    .map(|src| cx.line(src.value.syntax()))
                    .unwrap_or(line);
                (vname, value, vline)
            })
            .collect();
        for (i, (vname, value, vline)) in variants.iter().enumerate() {
            let sep = if i + 1 == variants.len() { ";" } else { "," };
            members.push(Member::Raw {
                line: *vline,
                text: format!("{vname}({value}){sep}"),
            });
        }
        members.extend(impl_members);
        if derives.iter().any(|d| d == "Debug") {
            trailer.push("public String debug() { return name(); }".to_string());
        }
        trailer.push("public final int value;".to_string());
        trailer.push(format!("{name}(int value) {{ this.value = value; }}"));
        header = format!("{vis}enum {name}{}", impl_clause(&implements));
    } else {
        let has_debug = derives.iter().any(|d| d == "Debug");
        let has_ord = derives.iter().any(|d| d == "PartialOrd" || d == "Ord");
        let has_display = impls
            .iter()
            .any(|imp| std_trait_name(cx, imp).as_deref() == Some("Display"));
        if has_debug {
            trailer.push("String debug();".to_string());
        }
        if derives.iter().any(|d| d == "Clone") {
            trailer.push(format!("{name} clone();"));
        }
        if has_ord {
            // Java's Comparable needs a method each record can inherit.
            trailer.push("int variantIndex();".to_string());
        }
        members.extend(impl_members);
        if let Some(en) = eh {
            for (i, v) in en.variants(cx.db).iter().enumerate() {
                members.push(Member::Class(lower_variant_record(
                    cx,
                    v,
                    &name,
                    &generics,
                    &generic_decls,
                    &derives,
                    line,
                    i,
                    has_display,
                    has_ord,
                )));
            }
        }
        if has_ord {
            let args = generic_arg_names(&generics);
            let iface = if args.is_empty() {
                name.clone()
            } else {
                format!("{name}<{}>", args.join(", "))
            };
            implements.push(format!("Comparable<{iface}>"));
        }
        header = format!(
            "{vis}sealed interface {name}{generics}{}",
            implements_clause(&implements, true)
        );
    }
    Class {
        line,
        header,
        members,
        close_line,
        trailer,
    }
}

fn empty_class(line: u32, close_line: u32, name: &str) -> Class {
    Class {
        line,
        header: format!("static final class {name}"),
        members: Vec::new(),
        close_line,
        trailer: Vec::new(),
    }
}

fn impl_clause(implements: &[String]) -> String {
    if implements.is_empty() {
        String::new()
    } else {
        format!(" implements {}", implements.join(", "))
    }
}

fn implements_clause(implements: &[String], is_interface: bool) -> String {
    if implements.is_empty() {
        String::new()
    } else if is_interface {
        format!(" extends {}", implements.join(", "))
    } else {
        impl_clause(implements)
    }
}

fn lower_variant_record(
    cx: &mut Cx,
    v: &ra_ap_hir::EnumVariant,
    enum_name: &str,
    enum_generics: &str,
    generic_decls: &[String],
    derives: &[String],
    fallback_line: u32,
    index: usize,
    has_display: bool,
    has_ord: bool,
) -> Class {
    let vname = escape_keyword(v.name(cx.db).as_str());
    let vline = v
        .source(cx.db)
        .map(|s| cx.line(s.value.syntax()))
        .unwrap_or(fallback_line);
    let close_line = v
        .source(cx.db)
        .map(|s| cx.end_line(s.value.syntax()))
        .unwrap_or(vline);
    let kind = v.kind(cx.db);
    let fields: Vec<FieldInfo> = match v.source(cx.db) {
        Some(src) => field_list_info(cx, src.value.field_list()),
        None => Vec::new(),
    };
    let generics = enum_generics.to_string();
    let type_args = generic_arg_names(enum_generics);
    let components: Vec<String> = fields
        .iter()
        .map(|f| format!("{} {}", f.ty.java(), f.name))
        .collect();
    let impl_iface = if type_args.is_empty() {
        enum_name.to_string()
    } else {
        format!("{enum_name}<{}>", type_args.join(", "))
    };
    let node = v.source(cx.db).map(|src| src.value.syntax().clone());
    // Compare by variant index, then by fields: derived Ord/PartialOrd on a data enum.
    let derives_record: Vec<String> = derives
        .iter()
        .filter(|d| *d != "Ord" && *d != "PartialOrd")
        .cloned()
        .collect();
    let mut trailer = match &node {
        Some(node) => derive_methods(
            cx,
            node,
            &vname,
            generic_decls,
            kind,
            &fields,
            &derives_record,
            false,
        ),
        None => Vec::new(),
    };
    if has_display {
        trailer.push("public String toString() { return display(); }".to_string());
    }
    if has_ord {
        trailer.push(format!("public int variantIndex() {{ return {index}; }}"));
        let mut body = format!(
            "int c = Integer.compare({index}, other.variantIndex()); if (c != 0) return c; "
        );
        if !fields.is_empty() {
            body.push_str(&format!("{vname} o = ({vname}) other; "));
            let mut direct = false;
            for (i, f) in fields.iter().enumerate() {
                match compare_field(
                    &f.ty,
                    &format!("this.{}", f.name),
                    &format!("o.{}", f.name),
                    i,
                    i + 1 == fields.len(),
                ) {
                    Some((code, tail)) => {
                        direct = tail;
                        body.push_str(&code);
                    }
                    None => {
                        if let Some(node) = &node {
                            cx.error(
                                node,
                                &format!(
                                    "derive(Ord/PartialOrd): cannot compare field `{}`",
                                    f.name
                                ),
                            );
                        }
                    }
                }
            }
            if !direct {
                body.push_str("return 0;");
            }
        } else {
            body.push_str("return 0;");
        }
        trailer.push(format!(
            "public int compareTo({impl_iface} other) {{ {body} }}"
        ));
    }
    Class {
        line: vline,
        header: format!(
            "record {vname}{generics}({}) implements {impl_iface}",
            components.join(", ")
        ),
        members: Vec::new(),
        close_line,
        trailer,
    }
}

fn generic_arg_names(generics: &str) -> Vec<String> {
    let g = generics.trim();
    let inner = g
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or("");
    inner
        .split(',')
        .map(|p| {
            p.trim()
                .split(" extends ")
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .filter(|p| !p.is_empty())
        .collect()
}

// --- modules -----------------------------------------------------------------------------

fn lower_inline_module(cx: &mut Cx, m: &ast::Module) -> Member {
    let name = match cx.sema.to_def(m) {
        Some(module) => crate::frontend::module_class_name(cx.db, module),
        None => pascal_case(&m.name().map(|n| n.text().to_string()).unwrap_or_default()),
    };
    let vis = if m.visibility().is_some() {
        "public "
    } else {
        ""
    };
    let mut members = Vec::new();
    if let Some(list) = m.item_list() {
        let items: Vec<ast::Item> = list.items().collect();
        lower_items(cx, &items, &mut members);
    }
    Member::Class(Class {
        line: cx.line(m.syntax()),
        header: format!("{vis}static final class {name}"),
        members,
        close_line: cx.end_line(m.syntax()),
        trailer: Vec::new(),
    })
}

// --- traits ------------------------------------------------------------------------------

fn lower_trait(cx: &mut Cx, t: &ast::Trait) -> Option<Member> {
    let name = escape_keyword(&t.name()?.text().to_string());
    let vis = if t.visibility().is_some() {
        "public "
    } else {
        ""
    };
    let generics = generic_params(cx, t.generic_param_list(), t.where_clause());
    let mut supers = Vec::new();
    if let Some(bounds) = t.type_bound_list() {
        for b in bounds.bounds() {
            if let Some(bound) = bound_java(cx, &b, &name)
                && !bound.starts_with("Comparable")
            {
                supers.push(bound);
            }
        }
    }
    let extends = if supers.is_empty() {
        String::new()
    } else {
        format!(" extends {}", supers.join(", "))
    };
    let saved_self = std::mem::replace(&mut cx.self_ty_java, name.clone());
    let mut members = Vec::new();
    if let Some(list) = t.assoc_item_list() {
        for item in list.assoc_items() {
            match item {
                ast::AssocItem::Fn(f) => {
                    // Java interfaces cannot declare abstract static methods; concrete types get
                    // the impl's static method instead.
                    if f.param_list().and_then(|p| p.self_param()).is_none() {
                        continue;
                    }
                    let modifiers = if f.body().is_some() { "default " } else { "" };
                    members.push(Member::Method(lower_fn(cx, &f, modifiers)));
                }
                ast::AssocItem::Const(c) => {
                    if let Some(m) = lower_const(cx, c.name(), c.ty(), c.body(), c.syntax()) {
                        members.push(m);
                    }
                    members.extend(const_accessor(cx, &c, &name, true));
                }
                _ => {}
            }
        }
    }
    cx.self_ty_java = saved_self;
    Some(Member::Class(Class {
        line: cx.line(t.syntax()),
        header: format!("{vis}interface {name}{generics}{extends}"),
        members,
        close_line: cx.end_line(t.syntax()),
        trailer: Vec::new(),
    }))
}

// --- impls -------------------------------------------------------------------------------

pub(crate) fn impl_trait(cx: &Cx, imp: &ast::Impl) -> Option<Trait> {
    cx.sema.to_impl_def(imp)?.trait_(cx.db)
}

fn lower_impls(cx: &mut Cx, impls: &[&ast::Impl], target: &Target) -> (Vec<Member>, Vec<String>) {
    let mut members = Vec::new();
    let mut implements = Vec::new();
    // `impl Ord` wins over `impl PartialOrd` for the generated `compareTo`.
    let has_ord = impls
        .iter()
        .any(|imp| std_trait_name(cx, imp).as_deref() == Some("Ord"));
    for imp in impls {
        let tr = impl_trait(cx, imp);
        let mut tname = String::new();
        if let Some(tr) = tr {
            let local = tr.module(cx.db).krate(cx.db).origin(cx.db).is_local();
            tname = tr.name(cx.db).as_str().to_string();
            if local {
                implements.push(trait_java_name(cx.db, cx.module, tr));
            } else {
                match tname.as_str() {
                    "Display" => {
                        let header = if target.interface {
                            "default String display()"
                        } else {
                            "public String toString()"
                        };
                        if let Some(m) = write_method(cx, imp, target, header) {
                            members.push(Member::Method(m));
                        }
                        continue;
                    }
                    "Debug" => {
                        let header = if target.interface {
                            "default String debug()"
                        } else {
                            "public String debug()"
                        };
                        if let Some(m) = write_method(cx, imp, target, header) {
                            members.push(Member::Method(m));
                        }
                        continue;
                    }
                    "PartialEq" | "Eq" | "Hash" => {
                        // Java forbids a default method that overrides Object's equals/hashCode.
                        if target.interface {
                            cx.error(
                                imp.syntax(),
                                "manual PartialEq/Eq/Hash on a data enum is not supported \
                                 (its records already implement equals/hashCode)",
                            );
                        } else if tname == "PartialEq" {
                            if let Some(m) = equals_impl(cx, imp, target) {
                                members.push(Member::Method(m));
                            }
                        } else if tname == "Hash"
                            && let Some(m) = hash_code_impl(cx, imp, target)
                        {
                            members.push(Member::Method(m));
                        }
                        continue;
                    }
                    _ => {}
                }
            }
        }
        let Some(list) = imp.assoc_item_list() else {
            continue;
        };
        let index_mut = tname == "IndexMut";
        for item in list.assoc_items() {
            match item {
                ast::AssocItem::Fn(f) => {
                    let fname = f.name().map(|n| n.text().to_string()).unwrap_or_default();
                    let is_static = f.param_list().and_then(|p| p.self_param()).is_none();
                    let vis = if f.visibility().is_some() {
                        "public "
                    } else {
                        ""
                    };
                    let trait_impl = tr.is_some();
                    let static_generics: &[String] = if is_static { &target.generics } else { &[] };
                    let modifiers = if is_static {
                        if trait_impl {
                            "public static ".to_string()
                        } else {
                            format!("{vis}static ")
                        }
                    } else if target.interface {
                        "default ".to_string()
                    } else if trait_impl {
                        "public ".to_string()
                    } else {
                        vis.to_string()
                    };
                    // Associated types in signatures (`Self::Item`, `Self::Err`) are not
                    // resolved by rust-analyzer's type conversion; spell them out here.
                    let ret_override = if tname == "Iterator" && fname == "next" {
                        traits::impl_assoc_type(cx, imp, "Item")
                            .map(|item| Ty::Option(Box::new(item)))
                    } else if matches!(tname.as_str(), "FromStr" | "TryFrom") {
                        let assoc = if tname == "FromStr" { "Err" } else { "Error" };
                        traits::impl_assoc_type(cx, imp, assoc).map(|err| {
                            Ty::Result(
                                Box::new(Ty::Adt {
                                    name: target.java.clone(),
                                    java: target.java.clone(),
                                    args: Vec::new(),
                                }),
                                Box::new(err),
                            )
                        })
                    } else {
                        None
                    };
                    // `index_mut` itself cannot be expressed (Java has no mutable element
                    // references); only the generated `index_mut_set` setter is needed.
                    if index_mut && fname == "index_mut" {
                        if let Some(setter) = index_mut_setter(cx, &f, target) {
                            members.push(Member::Method(setter));
                        }
                        continue;
                    }
                    let saved = std::mem::replace(&mut cx.self_ty_java, target.java.clone());
                    let m = lower_fn_sig(cx, &f, &modifiers, static_generics, None, ret_override);
                    cx.self_ty_java = saved;
                    members.push(Member::Method(m));
                }
                ast::AssocItem::Const(c) => {
                    if let Some(m) = lower_const(cx, c.name(), c.ty(), c.body(), c.syntax()) {
                        members.push(m);
                    }
                    members.extend(const_accessor(cx, &c, &tname, false));
                }
                _ => {}
            }
        }
        if tname == "Iterator"
            && let Some(item) = traits::impl_assoc_type(cx, imp, "Item")
        {
            members.push(Member::Method(stream_method(cx, imp, &item)));
        }
        if matches!(tname.as_str(), "Ord" | "PartialOrd") {
            let is_ord = tname == "Ord";
            if (is_ord || !has_ord)
                && let Some(m) = compare_to_method(cx, imp, target, is_ord)
            {
                if !implements.iter().any(|i| i.starts_with("Comparable")) {
                    implements.push(format!("Comparable<{}>", target.java));
                }
                members.push(Member::Method(m));
            }
        }
    }
    (members, implements)
}

fn std_trait_name(cx: &Cx, imp: &ast::Impl) -> Option<String> {
    let tr = impl_trait(cx, imp)?;
    (!tr.module(cx.db).krate(cx.db).origin(cx.db).is_local())
        .then(|| tr.name(cx.db).as_str().to_string())
}

fn find_impl_fn(imp: &ast::Impl, name: &str) -> Option<ast::Fn> {
    imp.assoc_item_list()?.assoc_items().find_map(|i| match i {
        ast::AssocItem::Fn(f) if f.name().is_some_and(|n| n.text() == name) => Some(f),
        _ => None,
    })
}

// Java type of the second parameter of `eq`/`cmp` (`&Self` included).
fn other_param_java(cx: &mut Cx, param: &ast::Param, target: &Target) -> String {
    match param.ty() {
        Some(t) if !t.syntax().text().to_string().contains("Self") => {
            let j = cx.type_ref(&t).strip_refs().java();
            if j == "Object" {
                target.java.clone()
            } else {
                j
            }
        }
        _ => target.java.clone(),
    }
}

// `impl PartialEq for S` -> `equals(Object)` that casts and runs the Rust `eq` body.
fn equals_impl(cx: &mut Cx, imp: &ast::Impl, target: &Target) -> Option<Method> {
    let f = find_impl_fn(imp, "eq")?;
    let params: Vec<ast::Param> = f.param_list()?.params().collect();
    let param = params.first()?;
    let ast::Pat::IdentPat(ip) = param.pat()? else {
        cx.unsupported(f.syntax(), "`eq` parameter that is not a plain identifier");
        return None;
    };
    if ip.pat().is_some() {
        cx.unsupported(f.syntax(), "`eq` parameter pattern");
        return None;
    }
    let rust_name = ip.name()?.text().to_string();
    let other_ty = other_param_java(cx, param, target);
    let line = cx.line(f.syntax());
    let close_line = cx.end_line(f.syntax());
    let saved_self = std::mem::replace(&mut cx.self_ty_java, target.java.clone());
    let saved = cx.save_method_state();
    cx.enter_method();
    let obj = cx.declare("o");
    let other = cx.declare(&rust_name);
    let mut prelude = vec![Stmt::new(
        line,
        StmtKind::If {
            cond: Expr::Unary(
                "!",
                Box::new(Expr::InstanceOf(
                    Box::new(Expr::name(obj.clone())),
                    format!("{other_ty} {other}"),
                )),
            ),
            then: crate::jir::Block {
                stmts: vec![Stmt::new(line, StmtKind::Return(Some(Expr::raw("false"))))],
                close_line: line,
            },
            els: None,
        },
    )];
    cx.ret_ty = Some(Ty::Bool);
    let body = f.body()?;
    let mut block = stmt::lower_block(cx, &body, &Dest::Return);
    prelude.append(&mut block.stmts);
    block.stmts = prelude;
    block.close_line = close_line;
    cx.ret_ty = None;
    cx.restore_method_state(saved);
    cx.self_ty_java = saved_self;
    Some(Method {
        line,
        header: format!("public boolean equals(Object {obj})"),
        body: Some(block),
    })
}

// `impl Hash` -> `hashCode()` over the `self.f.hash(state)` calls of the Rust body.
fn hash_code_impl(cx: &mut Cx, imp: &ast::Impl, target: &Target) -> Option<Method> {
    let f = find_impl_fn(imp, "hash")?;
    let line = cx.line(f.syntax());
    let close_line = cx.end_line(f.syntax());
    let saved_self = std::mem::replace(&mut cx.self_ty_java, target.java.clone());
    let saved = cx.save_method_state();
    cx.enter_method();
    let mut inputs: Vec<Expr> = Vec::new();
    let mut ok = true;
    if let Some(list) = f.body().and_then(|b| b.stmt_list()) {
        if list.tail_expr().is_some() {
            ok = false;
        }
        for s in list.statements() {
            let ast::Stmt::ExprStmt(es) = s else {
                ok = false;
                break;
            };
            let Some(ast::Expr::MethodCallExpr(m)) = es.expr() else {
                ok = false;
                break;
            };
            let name = m
                .name_ref()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            let mut out = Vec::new();
            let value = if name == "hash" {
                m.receiver().map(|r| expr::lower_expr(cx, &r, &mut out))
            } else if name.starts_with("write_") {
                m.arg_list()
                    .and_then(|a| a.args().next())
                    .map(|a| expr::lower_expr(cx, &a, &mut out))
            } else {
                None
            };
            match value {
                Some(v) if out.is_empty() => inputs.push(v),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
    } else {
        ok = false;
    }
    cx.restore_method_state(saved);
    cx.self_ty_java = saved_self;
    if !ok {
        cx.error(
            f.syntax(),
            "manual Hash impl must only hash fields (`self.f.hash(state)`)",
        );
        return None;
    }
    let call = Expr::call(Expr::raw("Objects"), "hash", inputs);
    Some(Method {
        line,
        header: "public int hashCode()".into(),
        body: Some(crate::jir::Block {
            stmts: vec![Stmt::new(
                line,
                StmtKind::Return(Some(Expr::raw(crate::emit::render_expr(&call)))),
            )],
            close_line,
        }),
    })
}

// `impl Ord`/`impl PartialOrd` -> `compareTo` delegating to the Rust-named method.
fn compare_to_method(
    cx: &mut Cx,
    imp: &ast::Impl,
    target: &Target,
    is_ord: bool,
) -> Option<Method> {
    let fname = if is_ord { "cmp" } else { "partial_cmp" };
    let f = find_impl_fn(imp, fname)?;
    let params: Vec<ast::Param> = f.param_list()?.params().collect();
    let param = params.first()?;
    let other_ty = other_param_java(cx, param, target);
    let other = match param.pat() {
        Some(ast::Pat::IdentPat(ip)) if ip.pat().is_none() => ip
            .name()
            .map(|n| escape_keyword(&n.text().to_string()))
            .unwrap_or_else(|| "other".into()),
        _ => "other".into(),
    };
    // Printed after the Rust-named method, so it belongs on that method's closing line.
    let line = cx.end_line(f.syntax());
    let call = Expr::raw(format!(
        "{}({other}){}",
        escape_keyword(fname),
        if is_ord { "" } else { ".orElse(0)" }
    ));
    Some(Method {
        line,
        header: format!("public int compareTo({other_ty} {other})"),
        body: Some(crate::jir::Block {
            stmts: vec![Stmt::new(line, StmtKind::Return(Some(call)))],
            close_line: cx.end_line(f.syntax()),
        }),
    })
}

// `impl IndexMut` needs a setter: Java cannot assign through a method call.
fn index_mut_setter(cx: &mut Cx, f: &ast::Fn, target: &Target) -> Option<Method> {
    let Some(place) = f
        .body()
        .and_then(|b| b.stmt_list())
        .and_then(|l| l.tail_expr())
        .and_then(|e| match super::strip_parens(&e) {
            ast::Expr::RefExpr(r) => r.expr(),
            _ => None,
        })
    else {
        cx.unsupported(f.syntax(), "impl IndexMut whose body is not `&mut <place>`");
        return None;
    };
    let out_ty = cx
        .sema
        .to_fn_def(f)
        .map(|d| cx.convert(&d.ret_type(cx.db)).strip_refs().java())
        .unwrap_or_else(|| "Object".into());
    let line = cx.line(f.syntax());
    let saved_self = std::mem::replace(&mut cx.self_ty_java, target.java.clone());
    let saved = cx.save_method_state();
    cx.enter_method();
    let mut jparams: Vec<String> = Vec::new();
    let mut prelude = Vec::new();
    for p in f.param_list().into_iter().flat_map(|pl| pl.params()) {
        let ty = p
            .ty()
            .map(|t| cx.type_ref(&t))
            .unwrap_or(Ty::Other("?".into()));
        let jty = param_type(cx, &ty);
        match p.pat() {
            Some(ast::Pat::IdentPat(ip)) if ip.pat().is_none() => {
                let n = ip
                    .name()
                    .map(|n| n.text().to_string())
                    .unwrap_or_else(|| "_".into());
                jparams.push(format!("{jty} {}", cx.declare(&n)));
            }
            // `(r, c): (usize, usize)`: bound from the parameter like an ordinary fn's.
            Some(other) => {
                let tmp = cx.fresh("arg");
                jparams.push(format!("{jty} {tmp}"));
                super::pat::bind_pattern(cx, &other, Expr::name(tmp), &mut prelude);
            }
            None => {}
        }
    }
    let value = cx.fresh("value");
    jparams.push(format!("{out_ty} {value}"));
    let assign = expr::place_assign(cx, &place, "=", Expr::name(value), &mut prelude);
    cx.restore_method_state(saved);
    cx.self_ty_java = saved_self;
    prelude.push(Stmt::new(
        line,
        if assign.is_statement_expr() {
            StmtKind::Expr(assign)
        } else {
            StmtKind::Raw(crate::emit::render_expr(&assign) + ";")
        },
    ));
    Some(Method {
        line,
        header: format!("public void index_mut_set({})", jparams.join(", ")),
        body: Some(crate::jir::Block {
            stmts: prelude,
            close_line: cx.end_line(f.syntax()),
        }),
    })
}

// `impl Iterator` -> `stream()` over `next()`, so for loops and stream pipelines work.
fn stream_method(cx: &mut Cx, imp: &ast::Impl, item: &Ty) -> Method {
    let line = cx.end_line(imp.syntax());
    let ret = Expr::raw("Stream.generate(() -> next().orElse(null)).takeWhile(Objects::nonNull)");
    Method {
        line,
        header: format!("public Stream<{}> stream()", item.boxed()),
        body: Some(crate::jir::Block {
            stmts: vec![Stmt::new(line, StmtKind::Return(Some(ret)))],
            close_line: cx.end_line(imp.syntax()),
        }),
    }
}

// `impl Display`/`impl Debug`: `write!(f, ..)` appends to a StringBuilder returned as a String.
fn write_method(cx: &mut Cx, imp: &ast::Impl, target: &Target, header: &str) -> Option<Method> {
    let list = imp.assoc_item_list()?;
    let fmt_fn = list.assoc_items().find_map(|i| match i {
        ast::AssocItem::Fn(f) if f.name().is_some_and(|n| n.text() == "fmt") => Some(f),
        _ => None,
    })?;
    let line = cx.line(fmt_fn.syntax());
    let saved_self = std::mem::replace(&mut cx.self_ty_java, target.java.clone());
    let saved_display = std::mem::replace(&mut cx.in_display, true);
    cx.enter_method();
    let fname = cx.declare("f");
    let mut stmts = vec![Stmt::new(
        line,
        StmtKind::Local {
            ty: "var".into(),
            name: fname.clone(),
            init: Some(Expr::New {
                ty: "StringBuilder".into(),
                args: Vec::new(),
            }),
        },
    )];
    if let Some(body) = fmt_fn.body()
        && let Some(text) = body.stmt_list()
    {
        for s in text.statements() {
            stmt::lower_stmt(cx, &s, &mut stmts);
        }
        // `Ok(())` is the Java `return f.toString();` we add below.
        if let Some(tail) = text.tail_expr()
            && tail.syntax().text().to_string() != "Ok(())"
        {
            // `write!(f, ..)` returns the `Ok(())`; only its `f.append(..)` statement matters.
            match &tail {
                ast::Expr::MacroExpr(m) => {
                    let _ = super::macros::lower_macro_expr(cx, m, &mut stmts);
                }
                _ => stmt::lower_to_dest(cx, &tail, &Dest::Discard, &mut stmts),
            }
        }
    }
    let close_line = cx.end_line(fmt_fn.syntax());
    stmts.push(Stmt::new(
        close_line,
        StmtKind::Return(Some(Expr::raw(format!("{fname}.toString()")))),
    ));
    cx.in_display = saved_display;
    cx.self_ty_java = saved_self;
    Some(Method {
        line,
        header: header.to_string(),
        body: Some(crate::jir::Block { stmts, close_line }),
    })
}

// `impl Trait for ForeignType` -> static methods in a `<Trait>Impls` nested class.
fn lower_foreign_impl(cx: &mut Cx, imp: &ast::Impl) -> Option<Class> {
    let tr = impl_trait(cx, imp)?;
    let trait_name = escape_keyword(tr.name(cx.db).as_str());
    let class_name = format!("{trait_name}Impls");
    let line = cx.line(imp.syntax());
    let self_ty = cx.sema.to_impl_def(imp)?.self_ty(cx.db);
    let self_java = cx.convert(&self_ty).java();
    let impl_generics = generic_param_decls(cx, imp.generic_param_list(), imp.where_clause());
    let mut members = Vec::new();
    let list = imp.assoc_item_list()?;
    for item in list.assoc_items() {
        match item {
            ast::AssocItem::Fn(f) => {
                let self_param = f.param_list().and_then(|p| p.self_param());
                let is_static = self_param.is_none();
                if let Some(sp) = &self_param
                    && sp.amp_token().is_some()
                    && sp.mut_token().is_some()
                    && (cx.convert(&self_ty).is_primitive() || cx.convert(&self_ty).is_str())
                {
                    cx.error(
                        f.syntax(),
                        "`&mut self` on a foreign primitive/String type is not supported yet",
                    );
                    continue;
                }
                let saved_self_ty = std::mem::replace(&mut cx.self_ty_java, self_java.clone());
                let saved_self_name = std::mem::replace(&mut cx.self_name, "self".to_string());
                let extra: &[String] = if is_static { &[] } else { &impl_generics };
                let self_param = (!is_static).then(|| (self_java.clone(), "self".to_string()));
                let m = lower_fn_inner(cx, &f, "public static ", extra, self_param);
                cx.self_ty_java = saved_self_ty;
                cx.self_name = saved_self_name;
                members.push(Member::Method(m));
            }
            ast::AssocItem::Const(c) => {
                if let Some(m) = lower_const(cx, c.name(), c.ty(), c.body(), c.syntax()) {
                    members.push(m);
                }
            }
            _ => {}
        }
    }
    Some(Class {
        line,
        header: format!("static final class {class_name}"),
        members,
        close_line: cx.end_line(imp.syntax()),
        trailer: Vec::new(),
    })
}

// Java class holding the static methods of a trait impl on a foreign type.
fn foreign_impl_class(cx: &Cx, imp: Impl) -> Option<String> {
    let tr = imp.trait_(cx.db)?;
    let base = format!("{}Impls", escape_keyword(tr.name(cx.db).as_str()));
    let module = imp.module(cx.db);
    if module == cx.module {
        Some(base)
    } else {
        Some(format!(
            "{}.{}",
            module_class_from(cx.db, cx.module, module),
            base
        ))
    }
}

fn imp_is_foreign(cx: &Cx, imp: Impl) -> bool {
    match imp.self_ty(cx.db).strip_references().as_adt() {
        Some(adt) => !adt.module(cx.db).krate(cx.db).origin(cx.db).is_local(),
        None => true,
    }
}

// --- functions ---------------------------------------------------------------------------

// `modifiers` is e.g. "static " or "public " (with trailing space).
pub fn lower_fn(cx: &mut Cx, f: &ast::Fn, modifiers: &str) -> Method {
    lower_fn_inner(cx, f, modifiers, &[], None)
}

fn lower_fn_inner(
    cx: &mut Cx,
    f: &ast::Fn,
    modifiers: &str,
    extra_generics: &[String],
    self_param: Option<(String, String)>,
) -> Method {
    lower_fn_sig(cx, f, modifiers, extra_generics, self_param, None)
}

fn lower_fn_sig(
    cx: &mut Cx,
    f: &ast::Fn,
    modifiers: &str,
    extra_generics: &[String],
    self_param: Option<(String, String)>,
    ret_override: Option<Ty>,
) -> Method {
    let saved = cx.save_method_state();
    let line = cx.line(f.syntax());
    cx.enter_method();
    let name = f.name().map(|n| n.text().to_string()).unwrap_or_default();
    let def = cx.sema.to_fn_def(f);
    let ret = match ret_override {
        Some(t) => t,
        None => match def {
            Some(d) => cx.convert(&d.ret_type(cx.db)),
            None => Ty::Unit,
        },
    };
    let mut params = Vec::new();
    let mut prelude = Vec::new();
    // `F: Fn(i32) -> i32` parameters are the functional interface itself, so `f(x)` can call it.
    let fn_bounds = super::std_map::ptr::fn_bounds(cx, f);
    if let Some((ty, pname)) = &self_param {
        params.push(format!("{ty} {}", cx.declare(pname)));
    }
    if let Some(pl) = f.param_list() {
        for p in pl.params() {
            let ast_ty = p.ty();
            let ty = ast_ty
                .clone()
                .map(|t| cx.type_ref(&t))
                .unwrap_or(Ty::Other("?".into()));
            // `Cell<i32>`/`RefCell<i32>` parameters take the one-element array.
            let cell = ast_ty
                .as_ref()
                .and_then(|t| super::std_map::ptr::array_cell_type_ref(cx, t));
            super::std_map::ptr::note_fn(cx, p.syntax(), &ty);
            let bound = match &ty {
                Ty::Param(name) => fn_bounds.iter().find(|(n, _)| n == name).map(|(_, t)| t),
                _ => None,
            };
            if let Some(bound_ty) = bound {
                super::std_map::ptr::note_fn(cx, p.syntax(), bound_ty);
            }
            let jty = match &cell {
                Some(inner) => format!("{}[]", inner.java()),
                None => bound.map(Ty::java).unwrap_or_else(|| param_type(cx, &ty)),
            };
            match p.pat() {
                Some(ast::Pat::IdentPat(ip)) if ip.pat().is_none() => {
                    let n = ip
                        .name()
                        .map(|n| n.text().to_string())
                        .unwrap_or_else(|| "_".into());
                    params.push(format!("{jty} {}", cx.declare(&n)));
                    if cell.is_some() {
                        cx.mark_cell_handle(&ip);
                    } else if is_cell_param(&ty) {
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
    let mut generics = generic_param_decls(cx, f.generic_param_list(), f.where_clause());
    for g in extra_generics {
        if !generics.contains(g) {
            generics.push(g.clone());
        }
    }
    let generics = wrap_generics(&generics);
    super::std_map::ptr::note_fn(cx, f.syntax(), &ret);
    let ret_java = match super::std_map::ptr::array_cell_ret(cx, f) {
        Some(inner) => format!("{}[]", inner.java()),
        None if ret.is_unit() => "void".to_string(),
        None => ret.java(),
    };
    let java_name = match cx.sema.to_def(f) {
        Some(func) => method_java_name(cx, func),
        None => escape_keyword(&name),
    };
    let header = format!(
        "{modifiers}{generics}{ret_java} {java_name}({})",
        params.join(", ")
    );
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
    cx.restore_method_state(saved);
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
    matches!(ty, Ty::Ref { mutable: true, .. }) && super::is_mut_ref_cell_ty(ty)
}

fn wrap_generics(decls: &[String]) -> String {
    if decls.is_empty() {
        String::new()
    } else {
        format!("<{}> ", decls.join(", "))
    }
}

// `<T extends Comparable<T>>` for generic fns/structs/enums/traits (empty when not generic).
pub fn generic_params(
    cx: &mut Cx,
    list: Option<ast::GenericParamList>,
    where_clause: Option<ast::WhereClause>,
) -> String {
    let decls = generic_param_decls(cx, list, where_clause);
    if decls.is_empty() {
        String::new()
    } else {
        format!("<{}>", decls.join(", "))
    }
}

pub fn lower_local_item(cx: &mut Cx, item: &ast::Item, out: &mut Vec<Stmt>) {
    let _ = out;
    let mut members = Vec::new();
    match item {
        ast::Item::Use(_) => return,
        ast::Item::Fn(f) => members.push(Member::Method(lower_fn(cx, f, "private static "))),
        ast::Item::Const(c) => {
            if let Some(m) = lower_const(cx, c.name(), c.ty(), c.body(), c.syntax()) {
                members.push(m);
            }
        }
        ast::Item::Struct(s) => members.push(Member::Class(lower_struct(cx, s, None, Vec::new()))),
        _ => {
            cx.unsupported(item.syntax(), "item inside a function body");
            return;
        }
    }
    for m in &members {
        let text = crate::emit::render_member_flat(m);
        cx.file_trailer.push(text);
    }
}

// --- structs: literals and constructors ---------------------------------------------------

pub fn lower_record_expr(cx: &mut Cx, r: &ast::RecordExpr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(r.syntax());
    let Some(variant) = cx.sema.resolve_variant(r.clone()) else {
        return cx.unsupported(r.syntax(), "struct literal");
    };
    let adt = variant.adt(cx.db);
    let target = match adt {
        Adt::Struct(_) => adt_java_name(cx, adt),
        Adt::Enum(_) => match variant {
            Variant::EnumVariant(v) => variant_java_name(cx, v),
            _ => return cx.unsupported(r.syntax(), "struct literal"),
        },
        Adt::Union(_) => return cx.unsupported(r.syntax(), "union literal"),
    };
    let fields = variant.fields(cx.db);
    let names = java_field_names(cx, &fields);
    let decl: Vec<String> = fields
        .iter()
        .map(|f| f.name(cx.db).as_str().to_string())
        .collect();
    let Some(list) = r.record_expr_field_list() else {
        return Expr::New {
            ty: target,
            args: Vec::new(),
        };
    };
    let spread = list.spread();
    let spread_impure = spread.as_ref().is_some_and(|e| !is_pure(e));
    let mut provided: Vec<(usize, Option<ast::Expr>, String)> = Vec::new();
    for f in list.fields() {
        let Some(name) = record_field_name(&f) else {
            continue;
        };
        let Some(idx) = decl.iter().position(|d| *d == name) else {
            return cx.unsupported(r.syntax(), "struct literal field");
        };
        // `Point { x }` shorthand: the value is the in-scope binding `x`.
        provided.push((idx, f.expr(), name));
    }
    let in_order = provided.windows(2).all(|w| w[0].0 < w[1].0);
    let impure = provided
        .iter()
        .any(|(_, e, _)| e.as_ref().is_some_and(|e| !is_pure(e)));
    // Rust evaluates the written fields in order, then the `..base` expression.
    let hoist_fields = (!in_order && impure) || (spread_impure && !provided.is_empty());
    let mut hoisted: Vec<(usize, Expr)> = Vec::new();
    if hoist_fields {
        for (idx, e, name) in &provided {
            let want = cx.convert(&fields[*idx].ty(cx.db));
            let v = match e {
                Some(e) => expr::lower_expr_as(cx, e, &want, out),
                None => Expr::name(cx.lookup(name)),
            };
            let tmp = cx.fresh("fld");
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: stmt::decl_type(&want),
                    name: tmp.clone(),
                    init: Some(v),
                },
            ));
            hoisted.push((*idx, Expr::name(tmp)));
        }
    }
    let mut base: Option<Expr> = None;
    if let Some(spread) = spread {
        let v = expr::lower_expr(cx, &spread, out);
        let v = if spread_impure {
            let tmp = cx.fresh("base");
            let ty = cx.ty(&spread);
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: stmt::decl_type(&ty),
                    name: tmp.clone(),
                    init: Some(v),
                },
            ));
            Expr::name(tmp)
        } else {
            v
        };
        base = Some(v);
    }
    let mut args = Vec::new();
    for (idx, f) in fields.iter().enumerate() {
        let want = cx.convert(&f.ty(cx.db));
        let field_value = |v: Expr| match &want {
            Ty::Iter(_) => Expr::call(Expr::call(v, "toList", vec![]), "listIterator", vec![]),
            _ => v,
        };
        if let Some((_, v)) = hoisted.iter().find(|(i, _)| *i == idx) {
            args.push(field_value(v.clone()));
        } else if let Some((_, e, pname)) = provided.iter().find(|(i, _, _)| *i == idx) {
            args.push(field_value(match e {
                Some(e) => expr::lower_expr_as(cx, e, &want, out),
                None => Expr::name(cx.lookup(pname)),
            }));
        } else if let Some(b) = &base {
            args.push(Expr::Field(Box::new(b.clone()), names[idx].clone()));
        } else {
            return cx.unsupported(r.syntax(), "struct literal missing field");
        }
    }
    let ty = cx.ty(&ast::Expr::RecordExpr(r.clone()));
    Expr::New {
        ty: diamond(cx, &target, &ty),
        args,
    }
}

fn diamond(cx: &Cx, target: &str, ty: &Ty) -> String {
    match ty.strip_refs() {
        // Explicit arguments: javac cannot infer a recursive bound like `T extends Comparable<T>`.
        Ty::Adt { args, .. } if !args.is_empty() => {
            let _ = cx;
            let rendered: Vec<String> = args.iter().map(|a| a.boxed()).collect();
            if rendered.iter().any(|a| a.contains('?')) {
                format!("{target}<>")
            } else {
                format!("{target}<{}>", rendered.join(", "))
            }
        }
        _ => target.to_string(),
    }
}

fn record_field_name(f: &ast::RecordExprField) -> Option<String> {
    if let Some(n) = f.name_ref() {
        return Some(n.text().to_string());
    }
    // `Point { x }` parses as a bare path expression.
    match f.expr()? {
        ast::Expr::PathExpr(p) => p
            .path()?
            .segment()?
            .name_ref()
            .map(|n| n.text().to_string()),
        _ => None,
    }
}

// Path in value position resolving to an item (const, static, fn value, unit variant/struct).
pub fn lower_def_path(cx: &mut Cx, path: &ast::Path, def: ModuleDef) -> Expr {
    let db = cx.db;
    let local = def
        .module(db)
        .is_some_and(|m| m.krate(db).origin(db).is_local());
    if !local {
        return std_map::lower_std_path(cx, path, def);
    }
    let qualify = |cx: &Cx, name: String| -> Expr {
        match def.module(cx.db) {
            Some(m) if m != cx.module => {
                Expr::raw(format!("{}.{name}", module_class_from(cx.db, cx.module, m)))
            }
            _ => Expr::raw(name),
        }
    };
    match def {
        ModuleDef::Const(c) => {
            let n = escape_keyword(
                &c.name(db)
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_default(),
            );
            // Associated consts live on their type's class or the trait interface.
            if let Some(assoc) = c.as_assoc_item(db) {
                match assoc.container(db) {
                    AssocItemContainer::Impl(imp) => {
                        let owner = if imp_is_foreign(cx, imp) {
                            match foreign_impl_class(cx, imp) {
                                Some(class) => class,
                                None => cx.self_ty_java.clone(),
                            }
                        } else {
                            impl_owner_java(cx, imp)
                        };
                        return Expr::raw(format!("{owner}.{n}"));
                    }
                    AssocItemContainer::Trait(tr) => {
                        if let Some(e) = trait_const_via_value(cx, path, &n) {
                            return e;
                        }
                        return Expr::raw(format!("{}.{n}", trait_java_name(db, cx.module, tr)));
                    }
                }
            }
            qualify(cx, n)
        }
        ModuleDef::Static(s) => qualify(cx, escape_keyword(s.name(db).as_str())),
        ModuleDef::Function(f) => {
            let name = escape_keyword(f.name(db).as_str());
            // A method reference's qualifier is the type for impl methods, else the module class.
            if let Some(assoc) = f.as_assoc_item(db)
                && let AssocItemContainer::Impl(imp) = assoc.container(db)
            {
                return Expr::raw(format!("{}::{name}", impl_owner_java(cx, imp)));
            }
            let owner = match def.module(db) {
                Some(m) if m != cx.module && m.name(db).is_some() => {
                    module_class_from(db, cx.module, m)
                }
                _ => cx.class_name.clone(),
            };
            Expr::raw(format!("{owner}::{name}"))
        }
        ModuleDef::EnumVariant(v) => {
            let e = v.parent_enum(db);
            if enum_is_plain(cx, e) {
                Expr::raw(variant_java_name(cx, v))
            } else if v.kind(db) == StructKind::Unit {
                Expr::New {
                    ty: variant_java_name(cx, v),
                    args: Vec::new(),
                }
            } else {
                let ctor = Expr::raw(format!("{}::new", variant_java_name(cx, v)));
                // As a function value it must produce the enum, not the variant record:
                // `.map(Command::Remove)` would otherwise infer `Result<Remove, ..>`.
                let fn_ty = path
                    .syntax()
                    .parent()
                    .and_then(ast::PathExpr::cast)
                    .map(|p| cx.ty(&ast::Expr::PathExpr(p)));
                match fn_ty {
                    Some(t @ Ty::Fn { .. }) if t.is_nameable() => {
                        Expr::Cast(t.java(), Box::new(ctor))
                    }
                    _ => ctor,
                }
            }
        }
        ModuleDef::Adt(Adt::Struct(s)) => {
            if s.kind(db) == StructKind::Unit {
                Expr::raw(format!("{}.INSTANCE", adt_java_name(cx, Adt::Struct(s))))
            } else {
                Expr::raw(format!("{}::new", adt_java_name(cx, Adt::Struct(s))))
            }
        }
        _ => cx.unsupported(path.syntax(), "path to item"),
    }
}

// Tuple-struct / tuple-variant constructor calls `Foo(a, b)`.
pub fn lower_ctor_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    path: &ast::Path,
    args: Vec<Expr>,
    out: &mut Vec<Stmt>,
) -> Expr {
    let _ = out;
    if let Some(e) = std_map::lower_std_ctor(cx, c, path, &args) {
        return e;
    }
    let target = match cx.sema.resolve_path(path) {
        Some(ra_ap_hir::PathResolution::Def(ModuleDef::Adt(Adt::Struct(s)))) => {
            adt_java_name(cx, Adt::Struct(s))
        }
        Some(ra_ap_hir::PathResolution::Def(ModuleDef::EnumVariant(v))) => variant_java_name(cx, v),
        _ => return cx.unsupported(c.syntax(), "tuple struct / variant constructor"),
    };
    let ty = cx.ty(&ast::Expr::CallExpr(c.clone()));
    Expr::New {
        ty: diamond(cx, &target, &ty),
        args,
    }
}

// Calls to `#[derive(..)]`-generated methods: rust-analyzer resolves them to a builtin impl
// that has no source node, while the generated Java method has a different name.
pub fn lower_derive_method_call(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    f: Function,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let AssocItemContainer::Impl(imp) = f.as_assoc_item(cx.db)?.container(cx.db) else {
        return None;
    };
    // Real impls have an AST; builtin derives do not.
    if imp.source(cx.db).is_some() {
        return None;
    }
    let tr = imp.trait_(cx.db)?;
    let tname = tr.name(cx.db).as_str().to_string();
    let name = m.name_ref()?.text().to_string();
    let plain_enum = imp
        .self_ty(cx.db)
        .strip_references()
        .as_adt()
        .is_some_and(|adt| adt.as_enum().is_some_and(|e| enum_is_plain(cx, e)));
    let recv = expr::lower_expr(cx, &m.receiver()?, out);
    let args = expr::call_args(cx, m, out);
    match (tname.as_str(), name.as_str()) {
        ("PartialEq", "eq") => Some(Expr::call(recv, "equals", args)),
        ("PartialEq", "ne") => Some(Expr::Unary("!", Box::new(Expr::call(recv, "equals", args)))),
        ("Ord" | "PartialOrd", "cmp") => Some(Expr::call(recv, "compareTo", args)),
        ("PartialOrd", "partial_cmp") => Some(Expr::call(
            Expr::raw("Optional"),
            "of",
            vec![Expr::call(recv, "compareTo", args)],
        )),
        ("Clone", "clone") if plain_enum => Some(recv),
        _ => None,
    }
}

// Method calls resolving to `impl LocalTrait for ForeignType`: dispatched statically.
pub fn lower_foreign_impl_call(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    f: Function,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let imp = match f.as_assoc_item(cx.db)?.container(cx.db) {
        AssocItemContainer::Impl(imp) => imp,
        AssocItemContainer::Trait(_) => return None,
    };
    if !imp_is_foreign(cx, imp) {
        return None;
    }
    let class = foreign_impl_class(cx, imp)?;
    let recv_ast = m.receiver()?;
    let recv = expr::lower_expr(cx, &recv_ast, out);
    let args = expr::call_args(cx, m, out);
    let name = escape_keyword(f.name(cx.db).as_str());
    Some(Expr::static_call(
        format!("{class}.{name}"),
        std::iter::once(recv).chain(args).collect(),
    ))
}

// Trait associated fns called on a concrete local type: `<Type>::method(..)`.
pub fn lower_trait_static_call(
    cx: &mut Cx,
    path: &ast::Path,
    f: Function,
    args: Vec<Expr>,
) -> Option<Expr> {
    let qualifier = path.qualifier()?;
    let owner = if qualifier.syntax().text().to_string() == "Self" {
        cx.self_ty_java.clone()
    } else {
        let Some(ra_ap_hir::PathResolution::Def(ModuleDef::Adt(adt))) =
            cx.sema.resolve_path(&qualifier)
        else {
            return None;
        };
        if !adt.module(cx.db).krate(cx.db).origin(cx.db).is_local() {
            return None;
        }
        adt_java_name(cx, adt)
    };
    let name = escape_keyword(f.name(cx.db).as_str());
    // `Default::default` has no impl method; the derived one is `default_`.
    let name = if name == "default" {
        "default_".to_string()
    } else {
        name
    };
    Some(Expr::static_call(format!("{owner}.{name}"), args))
}

pub fn lower_foreign_static_call(
    cx: &mut Cx,
    imp: Impl,
    name: &str,
    args: Vec<Expr>,
) -> Option<Expr> {
    let _ = cx;
    if !imp_is_foreign(cx, imp) {
        return None;
    }
    let class = foreign_impl_class(cx, imp)?;
    Some(Expr::static_call(format!("{class}.{name}"), args))
}

// Whether `e` names a `&mut` cell (one-element array).
pub fn is_cell(cx: &Cx, e: &ast::Expr) -> bool {
    let _ = (cx, e);
    false
}

// --- consts ------------------------------------------------------------------------------

// A trait's associated const read through a type parameter (`A::LEGS`) has no static owner in
// Java, so the trait declares an accessor method of the same name that each impl answers.
fn const_accessor(cx: &mut Cx, c: &ast::Const, trait_name: &str, in_trait: bool) -> Option<Member> {
    let rust_name = c.name()?.text().to_string();
    if cx.generic_consts.is_none() {
        cx.generic_consts = Some(generic_const_reads(cx));
    }
    if !cx
        .generic_consts
        .as_ref()
        .unwrap()
        .contains(&(trait_name.to_string(), rust_name.clone()))
    {
        return None;
    }
    let name = escape_keyword(&rust_name);
    let ty = cx.type_ref(&c.ty()?).java();
    let text = match (in_trait, c.body().is_some()) {
        (true, false) => format!("{ty} {name}();"),
        (true, true) => format!("default {ty} {name}() {{ return {name}; }}"),
        (false, _) => format!("public {ty} {name}() {{ return {name}; }}"),
    };
    Some(Member::Raw {
        line: cx.line(c.syntax()),
        text,
    })
}

// Trait consts some code reads as `A::NAME` / `Self::NAME`, as `(trait, const)` names.
fn generic_const_reads(cx: &Cx) -> std::collections::HashSet<(String, String)> {
    use ra_ap_hir::PathResolution;
    let mut found = std::collections::HashSet::new();
    let krate = cx.module.krate(cx.db);
    for module in krate.modules(cx.db) {
        let src = module.definition_source(cx.db);
        if !matches!(src.value, ra_ap_hir::ModuleSource::SourceFile(_)) {
            continue;
        }
        let root = cx.sema.parse(src.file_id.original_file(cx.db));
        // Expanded: the read is often a `format!` argument.
        for path in cx
            .expanded_descendants(root.syntax())
            .into_iter()
            .filter_map(ast::Path::cast)
        {
            let Some(q) = path.qualifier() else { continue };
            if !matches!(
                cx.sema.resolve_path(&q),
                Some(PathResolution::TypeParam(_) | PathResolution::SelfType(_))
            ) {
                continue;
            }
            if let Some(PathResolution::Def(ModuleDef::Const(c))) = cx.sema.resolve_path(&path)
                && let Some(assoc) = c.as_assoc_item(cx.db)
                && let AssocItemContainer::Trait(tr) = assoc.container(cx.db)
                && let Some(n) = c.name(cx.db)
            {
                found.insert((tr.name(cx.db).as_str().to_string(), n.as_str().to_string()));
            }
        }
    }
    found
}

// `A::LEGS` / `Self::LEGS` for a trait const: the accessor on a value of that type in scope.
fn trait_const_via_value(cx: &Cx, path: &ast::Path, name: &str) -> Option<Expr> {
    use ra_ap_hir::PathResolution;
    let qualifier = path.qualifier()?;
    match cx.sema.resolve_path(&qualifier)? {
        // Inside the trait, `Self` is its implicit type parameter.
        PathResolution::TypeParam(tp) if tp.name(cx.db).as_str() == "Self" => {
            Some(Expr::raw(format!("this.{name}()")))
        }
        PathResolution::TypeParam(tp) => {
            let f = cx.enclosing_fn(path.syntax())?;
            let f = ast::Fn::cast(f)?;
            let wanted = tp.name(cx.db).as_str().to_string();
            for p in f.param_list()?.params() {
                if let Some(ast::Pat::IdentPat(ip)) = p.pat()
                    && matches!(cx.pat_ty(&ast::Pat::IdentPat(ip.clone())).strip_refs(), Ty::Param(n) if *n == wanted)
                {
                    let java = cx.lookup(ip.name()?.text().as_ref());
                    return Some(Expr::raw(format!("{java}.{name}()")));
                }
            }
            None
        }
        PathResolution::SelfType(_) => Some(Expr::raw(format!("this.{name}()"))),
        _ => None,
    }
}

fn lower_const(
    cx: &mut Cx,
    name: Option<ast::Name>,
    ty: Option<ast::Type>,
    body: Option<ast::Expr>,
    node: &SyntaxNode,
) -> Option<Member> {
    let name = name?.text().to_string();
    let ty = ty.map(|t| cx.type_ref(&t)).unwrap_or(Ty::Other("?".into()));
    let body = body?;
    let saved = cx.save_method_state();
    cx.enter_method();
    let mut pre = Vec::new();
    let v = super::expr::lower_expr_as(cx, &body, &ty, &mut pre);
    cx.restore_method_state(saved);
    if !pre.is_empty() {
        cx.unsupported(node, "const initializer with statements");
    }
    Some(Member::Field {
        line: cx.line(node),
        text: format!(
            "static final {} {} = {};",
            ty.java(),
            escape_keyword(&name),
            crate::emit::render_expr(&v)
        ),
    })
}

// `thread_local! { static X: T = init; }`: the program has one thread, so `X` is a plain static
// field; an initializer with statements runs in a `static { }` block on its own lines.
fn lower_thread_local(cx: &mut Cx, mc: &ast::MacroCall, members: &mut Vec<Member>) {
    let Some(tt) = mc.token_tree() else { return };
    let text = tt.syntax().text().to_string();
    let base = tt.syntax().text_range().start() + ra_ap_syntax::TextSize::from(1);
    let parsed =
        ast::SourceFile::parse(&text[1..text.len() - 1], ra_ap_syntax::Edition::Edition2024);
    for item in parsed.tree().items() {
        let ast::Item::Static(s) = item else {
            cx.unsupported(mc.syntax(), "`thread_local!` item other than a static");
            continue;
        };
        let (Some(name), Some(body)) = (s.name(), s.body()) else {
            continue;
        };
        // The typed copy of the initializer lives in the macro expansion.
        let squash =
            |e: &SyntaxNode| -> String { e.text().to_string().split_whitespace().collect() };
        let want = squash(body.syntax());
        let init = body.syntax().first_token().and_then(|first| {
            let at = base + first.text_range().start();
            let orig = mc.syntax().token_at_offset(at).right_biased()?;
            cx.sema.descend_into_macros(orig).into_iter().find_map(|t| {
                t.parent_ancestors()
                    .filter_map(ast::Expr::cast)
                    .find(|e| squash(e.syntax()) == want)
            })
        });
        let Some(init) = init else {
            cx.unsupported(mc.syntax(), "`thread_local!` initializer");
            continue;
        };
        lower_static_init(cx, &escape_keyword(&name.text()), &init, members);
    }
}

// Java type where `Cell<T>` parts are their `T[]` arrays (the type model erases `Cell`).
fn java_type_with_cells<'db>(cx: &Cx<'_, 'db>, t: &ra_ap_hir::Type<'db>, boxed: bool) -> String {
    if let Some(inner) = super::std_map::ptr::array_cell(cx, t) {
        return format!("{}[]", inner.java());
    }
    let fields = t.tuple_fields(cx.db);
    match fields.len() {
        2 => format!(
            "Map.Entry<{}, {}>",
            java_type_with_cells(cx, &fields[0], true),
            java_type_with_cells(cx, &fields[1], true)
        ),
        _ => {
            let ty = cx.convert(t);
            if boxed { ty.boxed() } else { ty.java() }
        }
    }
}

fn lower_static_init(cx: &mut Cx, name: &str, init: &ast::Expr, members: &mut Vec<Member>) {
    let ty = cx.ty(init);
    let line = cx.line(init.syntax());
    let saved = cx.save_method_state();
    cx.enter_method();
    let mut stmts = Vec::new();
    let (value, value_line) = match init {
        ast::Expr::BlockExpr(b)
            if b.stmt_list()
                .is_some_and(|l| l.statements().next().is_some()) =>
        {
            let list = b.stmt_list().unwrap();
            for s in list.statements() {
                stmt::lower_stmt(cx, &s, &mut stmts);
            }
            match list.tail_expr() {
                Some(t) => (
                    super::expr::lower_expr_as(cx, &t, &ty, &mut stmts),
                    cx.line(t.syntax()),
                ),
                None => (Expr::raw("null"), cx.end_line(init.syntax())),
            }
        }
        _ => (super::expr::lower_expr_as(cx, init, &ty, &mut stmts), line),
    };
    cx.restore_method_state(saved);
    // Statics initialize in order, so the reader must be declared above the block using it.
    if let Some(i) = cx.file_trailer.iter().position(|t| t.contains(" STDIN = ")) {
        let text = cx.file_trailer.remove(i);
        cx.stdin_hoisted = true;
        members.push(Member::Field { line, text });
    }
    let ty_java = match cx.sema.type_of_expr(init) {
        Some(t) => java_type_with_cells(cx, &t.original, false),
        None => ty.java(),
    };
    if stmts.is_empty() {
        let text = format!(
            "static final {ty_java} {name} = {};",
            crate::emit::render_expr(&value)
        );
        members.push(Member::Field { line, text });
        return;
    }
    members.push(Member::Field {
        line,
        text: format!("static final {ty_java} {name};"),
    });
    stmts.push(Stmt::new(
        value_line,
        StmtKind::Expr(Expr::Assign(
            Box::new(Expr::name(name)),
            "=",
            Box::new(value),
        )),
    ));
    members.push(Member::Method(Method {
        line,
        header: "static".into(),
        body: Some(crate::jir::Block {
            stmts,
            close_line: cx.end_line(init.syntax()),
        }),
    }));
}

// --- generics ----------------------------------------------------------------------------

fn generic_param_decls(
    cx: &mut Cx,
    list: Option<ast::GenericParamList>,
    where_clause: Option<ast::WhereClause>,
) -> Vec<String> {
    let Some(list) = list else { return Vec::new() };
    let mut extra: Vec<(String, Vec<ast::TypeBound>)> = Vec::new();
    if let Some(wc) = where_clause {
        for pred in wc.predicates() {
            let Some(ast::Type::PathType(pt)) = pred.ty() else {
                continue;
            };
            let Some(path) = pt.path() else { continue };
            let name = path.syntax().text().to_string();
            let bounds = pred
                .type_bound_list()
                .map(|b| b.bounds().collect())
                .unwrap_or_default();
            extra.push((name, bounds));
        }
    }
    let mut out = Vec::new();
    for p in list.generic_params() {
        let ast::GenericParam::TypeParam(tp) = p else {
            continue;
        };
        let name = tp.name().map(|n| n.text().to_string()).unwrap_or_default();
        let mut bounds: Vec<String> = Vec::new();
        if let Some(bl) = tp.type_bound_list() {
            for b in bl.bounds() {
                if let Some(s) = bound_java(cx, &b, &name) {
                    bounds.push(s);
                }
            }
        }
        for (pname, preds) in &extra {
            if *pname == name {
                for b in preds {
                    if let Some(s) = bound_java(cx, b, &name)
                        && !bounds.contains(&s)
                    {
                        bounds.push(s);
                    }
                }
            }
        }
        if bounds.is_empty() {
            out.push(name);
        } else {
            out.push(format!("{name} extends {}", bounds.join(" & ")));
        }
    }
    out
}

fn bound_java(cx: &mut Cx, b: &ast::TypeBound, param: &str) -> Option<String> {
    if b.question_mark_token().is_some() {
        return None;
    }
    let ast::Type::PathType(pt) = b.ty()? else {
        return None;
    };
    let path = pt.path()?;
    let name = path.segment()?.name_ref()?.text().to_string();
    if name == "Sized" {
        return None;
    }
    if let Some(ra_ap_hir::PathResolution::Def(ModuleDef::Trait(tr))) = cx.sema.resolve_path(&path)
    {
        if tr.module(cx.db).krate(cx.db).origin(cx.db).is_local() {
            return Some(trait_java_name(cx.db, cx.module, tr));
        }
    }
    match name.as_str() {
        "Ord" | "PartialOrd" => Some(format!("Comparable<{param}>")),
        _ => None,
    }
}

fn member_line(m: &Member) -> u32 {
    match m {
        Member::Field { line, .. } | Member::Raw { line, .. } => *line,
        Member::Method(m) => m.line,
        Member::Class(c) => c.line,
    }
}

// `#[test]` / `#[cfg(test)]` items never reach the binary (and RA leaves their macros unexpanded).
fn is_test_only(item: &ast::Item) -> bool {
    item.attrs().any(|a| {
        a.as_simple_atom().is_some_and(|n| n == "test")
            || a.as_simple_call()
                .is_some_and(|(n, t)| n == "cfg" && t.syntax().text() == "(test)")
    })
}

// --- derives -----------------------------------------------------------------------------

fn derives_of(item: &impl HasAttrs) -> Vec<String> {
    let mut out = Vec::new();
    for attr in item.attrs() {
        let Some((name, tree)) = attr.as_simple_call() else {
            continue;
        };
        if name != "derive" {
            continue;
        }
        let text = tree.syntax().text().to_string();
        let inner = text.trim().trim_start_matches('(').trim_end_matches(')');
        for part in inner.split(',') {
            let seg = part.trim().rsplit("::").next().unwrap_or("").trim();
            if !seg.is_empty() {
                out.push(seg.to_string());
            }
        }
    }
    out
}

fn derive_methods(
    cx: &mut Cx,
    node: &SyntaxNode,
    name: &str,
    generic_decls: &[String],
    kind: StructKind,
    fields: &[FieldInfo],
    derives: &[String],
    is_struct: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    let has = |d: &str| derives.iter().any(|x| x == d);
    // Data-enum variants are Java records: their implicit equals/hashCode is component-wise,
    // which matches Rust's derive unless a component is a float (records compare bits).
    let record = !is_struct;
    let record_safe = record && fields.iter().all(|f| !f.ty.is_float());
    if has("Debug") {
        out.push(debug_method(cx, name, kind, fields));
    }
    if has("Clone") {
        let generics = wrap_generics(generic_decls);
        out.push(match kind {
            StructKind::Unit if is_struct => {
                format!("public {name} clone() {{ return INSTANCE; }}")
            }
            _ if record && fields.iter().all(|f| f.shallow_clone) => {
                format!("public {name}{generics} clone() {{ return this; }}")
            }
            _ => {
                let args: Vec<String> = fields
                    .iter()
                    .map(|f| {
                        if f.plain_enum {
                            format!("this.{}", f.name)
                        } else {
                            clone_value(&f.ty, &format!("this.{}", f.name))
                        }
                    })
                    .collect();
                format!(
                    "public {name}{generics} clone() {{ return new {name}({}); }}",
                    args.join(", ")
                )
            }
        });
    }
    if (has("PartialEq") || has("Eq")) && !record_safe {
        out.push(equals_method(name, fields));
    }
    if has("Hash") && !record_safe {
        out.push(hash_method(fields));
    }
    if has("Ord") || has("PartialOrd") {
        out.push(compare_method(cx, node, name, fields));
    }
    if has("Default") {
        out.push(default_method(cx, name, generic_decls, kind, fields));
    }
    out
}

fn this_field(f: &FieldInfo) -> Expr {
    Expr::Field(Box::new(Expr::raw("this")), f.name.clone())
}

enum DebugSeg {
    Lit(String),
    Val(Expr),
}

fn debug_method(cx: &mut Cx, name: &str, kind: StructKind, fields: &[FieldInfo]) -> String {
    // Debug prints the Rust name, not the escaped class name (`List_`).
    let name = match name.strip_suffix('_') {
        Some(rust) if escape_keyword(rust) == name => rust,
        _ => name,
    };
    let mut segs: Vec<DebugSeg> = Vec::new();
    let mut lit = String::new();
    match kind {
        StructKind::Unit => lit.push_str(name),
        StructKind::Record if fields.is_empty() => lit.push_str(name),
        StructKind::Tuple => {
            lit.push_str(&format!("{name}("));
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    lit.push_str(", ");
                }
                segs.push(DebugSeg::Lit(std::mem::take(&mut lit)));
                segs.push(DebugSeg::Val(debug_value(cx, &f.ty, this_field(f), 0)));
            }
            lit.push(')');
        }
        StructKind::Record => {
            lit.push_str(&format!("{name} {{ "));
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    lit.push_str(", ");
                }
                lit.push_str(&format!("{}: ", f.name));
                segs.push(DebugSeg::Lit(std::mem::take(&mut lit)));
                segs.push(DebugSeg::Val(debug_value(cx, &f.ty, this_field(f), 0)));
            }
            lit.push_str(" }");
        }
    }
    segs.push(DebugSeg::Lit(lit));
    let mut expr = str_concat(
        segs.iter()
            .map(|s| match s {
                DebugSeg::Lit(t) => Expr::str_lit(t),
                DebugSeg::Val(e) => e.clone(),
            })
            .collect(),
    );
    // `String.formatted` beats `+` once there are enough fields to pay for the varargs form.
    if fields.len() >= 3 {
        let template: String = segs
            .iter()
            .map(|s| match s {
                DebugSeg::Lit(t) => t.clone(),
                DebugSeg::Val(_) => "%s".to_string(),
            })
            .collect();
        let args: Vec<Expr> = segs
            .iter()
            .filter_map(|s| match s {
                DebugSeg::Val(e) => Some(e.clone()),
                DebugSeg::Lit(_) => None,
            })
            .collect();
        let formatted = Expr::call(Expr::str_lit(&template), "formatted", args);
        if crate::emit::render_expr(&formatted).len() < crate::emit::render_expr(&expr).len() {
            expr = formatted;
        }
    }
    format!(
        "public String debug() {{ return {}; }}",
        crate::emit::render_expr(&expr)
    )
}

// `debug_value` is a String except where Java's `+` would stringify it anyway.
fn debug_is_string(ty: &Ty) -> bool {
    match ty.strip_refs() {
        Ty::Int(i) => wide_unsigned(*i),
        Ty::Bool => false,
        Ty::Vec(t) => !crate::lower::fmt::debug_is_to_string(t),
        _ => true,
    }
}

// Debug text that must itself be a String, e.g. as a lambda body (not a `+` operand).
fn debug_string(cx: &mut Cx, ty: &Ty, v: Expr, depth: usize) -> Expr {
    let e = debug_value(cx, ty, v, depth);
    if debug_is_string(ty) {
        e
    } else {
        Expr::bin(Expr::str_lit(""), "+", e)
    }
}

// These need masking or an unsigned conversion, which `fmt::debug` supplies.
fn wide_unsigned(i: Int) -> bool {
    matches!(i, Int::U16 | Int::U32 | Int::U64 | Int::U128)
}

pub fn debug_value(cx: &mut Cx, ty: &Ty, v: Expr, depth: usize) -> Expr {
    if let Some(e) = crate::lower::fmt::std_error_debug(cx, v.clone(), ty) {
        return e;
    }
    match ty.strip_refs() {
        Ty::Str | Ty::Char => crate::lower::fmt::debug(cx, v, ty),
        Ty::Adt { .. } => Expr::call(v, "debug", vec![]),
        Ty::Option(inner) => {
            let x = format!("x{depth}");
            let body = str_concat(vec![
                Expr::str_lit("Some("),
                debug_value(cx, inner, Expr::name(&x), depth + 1),
                Expr::str_lit(")"),
            ]);
            let mapped = Expr::call(
                v,
                "map",
                vec![Expr::Lambda {
                    params: vec![x],
                    body: crate::jir::LambdaBody::Expr(Box::new(body)),
                }],
            );
            Expr::call(mapped, "orElse", vec![Expr::str_lit("None")])
        }
        Ty::Vec(inner) => debug_seq(cx, v, inner, depth, "[", "]"),
        Ty::Set { t, .. } => debug_seq(cx, v, t, depth, "{", "}"),
        Ty::Map { k, v: val, .. } => {
            let e = format!("e{depth}");
            let key = debug_value(
                cx,
                k,
                Expr::call(Expr::name(&e), "getKey", vec![]),
                depth + 1,
            );
            let value = debug_value(
                cx,
                val,
                Expr::call(Expr::name(&e), "getValue", vec![]),
                depth + 1,
            );
            let body = str_concat(vec![key, Expr::str_lit(": "), value]);
            let stream = Expr::call(v, "entrySet", vec![]);
            let stream = Expr::call(stream, "stream", vec![]);
            let mapped = Expr::call(
                stream,
                "map",
                vec![Expr::Lambda {
                    params: vec![e],
                    body: crate::jir::LambdaBody::Expr(Box::new(body)),
                }],
            );
            join_collect(mapped, "{", "}")
        }
        Ty::Tuple(ts) if ts.len() == 2 => str_concat(vec![
            Expr::str_lit("("),
            debug_value(cx, &ts[0], Expr::call(v.clone(), "getKey", vec![]), depth),
            Expr::str_lit(", "),
            debug_value(cx, &ts[1], Expr::call(v, "getValue", vec![]), depth),
            Expr::str_lit(")"),
        ]),
        Ty::Result(..) => Expr::call(v, "debug", vec![]),
        Ty::Unit => Expr::str_lit("()"),
        Ty::F32 | Ty::F64 => Expr::call(Expr::raw("String"), "valueOf", vec![v]),
        Ty::Param(_) => {
            // Generic Debug: at least match Rust for the common String/char payloads.
            let s1 = format!("s{depth}");
            let c1 = format!("c{depth}");
            let quoted_str = str_concat(vec![
                Expr::str_lit("\""),
                Expr::name(&s1),
                Expr::str_lit("\""),
            ]);
            let quoted_char = str_concat(vec![
                Expr::str_lit("'"),
                Expr::name(&c1),
                Expr::str_lit("'"),
            ]);
            let inner = Expr::Ternary(
                Box::new(Expr::InstanceOf(
                    Box::new(v.clone()),
                    format!("Character {c1}"),
                )),
                Box::new(quoted_char),
                Box::new(Expr::call(Expr::raw("String"), "valueOf", vec![v.clone()])),
            );
            Expr::Ternary(
                Box::new(Expr::InstanceOf(
                    Box::new(v.clone()),
                    format!("String {s1}"),
                )),
                Box::new(quoted_str),
                Box::new(inner),
            )
        }
        Ty::Int(i) if wide_unsigned(*i) => crate::lower::fmt::debug(cx, v, ty),
        Ty::Int(_) | Ty::Bool => v,
        _ => Expr::call(Expr::raw("String"), "valueOf", vec![v]),
    }
}

fn debug_seq(cx: &mut Cx, v: Expr, inner: &Ty, depth: usize, open: &str, close: &str) -> Expr {
    // A Java List of values whose `toString` is already Rust's Debug text needs no mapping.
    if open == "[" && crate::lower::fmt::debug_is_to_string(inner) {
        return v;
    }
    let x = format!("x{depth}");
    let body = debug_string(cx, inner, Expr::name(&x), depth + 1);
    let stream = Expr::call(v, "stream", vec![]);
    let mapped = Expr::call(
        stream,
        "map",
        vec![Expr::Lambda {
            params: vec![x],
            body: crate::jir::LambdaBody::Expr(Box::new(body)),
        }],
    );
    join_collect(mapped, open, close)
}

fn join_collect(stream: Expr, open: &str, close: &str) -> Expr {
    Expr::call(
        stream,
        "collect",
        vec![Expr::call(
            Expr::raw("Collectors"),
            "joining",
            vec![
                Expr::str_lit(", "),
                Expr::str_lit(open),
                Expr::str_lit(close),
            ],
        )],
    )
}

fn str_concat(parts: Vec<Expr>) -> Expr {
    let mut it = parts.into_iter();
    let Some(mut acc) = it.next() else {
        return Expr::str_lit("");
    };
    for p in it {
        acc = Expr::bin(acc, "+", p);
    }
    acc
}

fn equals_method(name: &str, fields: &[FieldInfo]) -> String {
    let body = if fields.is_empty() {
        format!("return o instanceof {name};")
    } else {
        let conds: Vec<String> = fields.iter().map(field_eq).collect();
        format!(
            "return o instanceof {name} other && {};",
            conds.join(" && ")
        )
    };
    format!("public boolean equals(Object o) {{ {body} }}")
}

// Rust's derive(PartialEq) uses `==` on primitives (floats included); references use equals.
fn field_eq(f: &FieldInfo) -> String {
    if f.ty.is_primitive() || f.ty.strip_refs() == &Ty::Ordering {
        format!("this.{0} == other.{0}", f.name)
    } else {
        format!("Objects.equals(this.{0}, other.{0})", f.name)
    }
}

fn hash_method(fields: &[FieldInfo]) -> String {
    let args: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
    format!(
        "public int hashCode() {{ return Objects.hash({}); }}",
        args.join(", ")
    )
}

fn compare_method(cx: &mut Cx, node: &SyntaxNode, name: &str, fields: &[FieldInfo]) -> String {
    let mut body = String::new();
    let last = fields.len().wrapping_sub(1);
    // A plain last field returns its comparison directly; the `cN` dance would need `return 0`.
    let mut direct = false;
    for (i, f) in fields.iter().enumerate() {
        match compare_field(
            &f.ty,
            &format!("this.{}", f.name),
            &format!("other.{}", f.name),
            i,
            i == last,
        ) {
            Some((code, tail)) => {
                direct = tail;
                body.push_str(&code);
            }
            None => cx.error(
                node,
                &format!("derive(Ord/PartialOrd): cannot compare field `{}`", f.name),
            ),
        }
    }
    if !direct {
        body.push_str("return 0;");
    }
    format!("public int compareTo({name} other) {{ {body} }}")
}

// One field's comparison, leaving its result in `cN` and returning early when it differs.
// `last` lets a plain comparison be returned directly, so no trailing `return 0` is needed.
fn compare_field(ty: &Ty, a: &str, b: &str, n: usize, last: bool) -> Option<(String, bool)> {
    match ty.strip_refs() {
        Ty::Vec(inner) => {
            let elem = compare_expr(inner, &format!("{a}.get(i{n})"), &format!("{b}.get(i{n})"))?;
            Some((
                format!(
                    "int c{n} = 0; for (int i{n} = 0; i{n} < Math.min({a}.size(), {b}.size()); i{n}++) {{ int d{n} = {elem}; if (d{n} != 0) return d{n}; }} c{n} = Integer.compare({a}.size(), {b}.size()); if (c{n} != 0) return c{n}; "
                ),
                false,
            ))
        }
        Ty::Option(inner) => {
            let elem = compare_expr(inner, &format!("{a}.get()"), &format!("{b}.get()"))?;
            Some((
                format!(
                    "int c{n} = 0; if ({a}.isPresent() && {b}.isPresent()) {{ c{n} = {elem}; }} else if ({a}.isPresent()) {{ c{n} = 1; }} else if ({b}.isPresent()) {{ c{n} = -1; }} if (c{n} != 0) return c{n}; "
                ),
                false,
            ))
        }
        _ => {
            let cmp = compare_expr(ty, a, b)?;
            if last {
                Some((format!("return {cmp};"), true))
            } else {
                Some((
                    format!("int c{n} = {cmp}; if (c{n} != 0) return c{n}; "),
                    false,
                ))
            }
        }
    }
}

fn compare_expr(ty: &Ty, a: &str, b: &str) -> Option<String> {
    Some(match ty.strip_refs() {
        Ty::Int(i) if i.is_long() => format!("Long.compare({a}, {b})"),
        Ty::Int(_) => format!("Integer.compare({a}, {b})"),
        Ty::F32 => format!("Float.compare({a}, {b})"),
        Ty::F64 => format!("Double.compare({a}, {b})"),
        Ty::Bool => format!("Boolean.compare({a}, {b})"),
        Ty::Char => format!("Character.compare({a}, {b})"),
        Ty::Str => format!("{a}.compareTo({b})"),
        Ty::Adt { .. } | Ty::Param(_) => format!("{a}.compareTo({b})"),
        _ => return None,
    })
}

// Deep copy where Rust's derive(Clone) would copy a container.
fn clone_value(ty: &Ty, v: &str) -> String {
    match ty.strip_refs() {
        Ty::Vec(_) => format!("new ArrayList<>({v})"),
        Ty::Map { ordered, .. } => {
            format!("new {}({v})", if *ordered { "TreeMap" } else { "HashMap" })
        }
        Ty::Set { ordered, .. } => {
            format!("new {}({v})", if *ordered { "TreeSet" } else { "HashSet" })
        }
        Ty::Deque(_) => format!("new ArrayDeque<>({v})"),
        Ty::Adt { .. } => format!("{v}.clone()"),
        _ => v.to_string(),
    }
}

fn default_method(
    cx: &mut Cx,
    name: &str,
    generic_decls: &[String],
    kind: StructKind,
    fields: &[FieldInfo],
) -> String {
    let generics = wrap_generics(generic_decls);
    if kind == StructKind::Unit {
        return format!("public static {generics}{name} default_() {{ return INSTANCE; }}");
    }
    let args: Vec<String> = fields.iter().map(|f| default_value(cx, &f.ty)).collect();
    format!(
        "public static {generics}{name} default_() {{ return new {name}({}); }}",
        args.join(", ")
    )
}

pub(crate) fn default_value(cx: &mut Cx, ty: &Ty) -> String {
    // Ordered collections of keys Java can't compare need the Rust `Ord` comparator.
    if let Some(e) = super::std_map::ordered_new(cx, ty) {
        return crate::emit::render_expr(&e);
    }
    match ty.strip_refs() {
        Ty::Int(i) if i.is_long() => "0L".into(),
        Ty::Int(_) => "0".into(),
        Ty::F32 => "0.0f".into(),
        Ty::F64 => "0.0".into(),
        Ty::Bool => "false".into(),
        Ty::Char => "'\\0'".into(),
        Ty::Str => "\"\"".into(),
        Ty::Vec(_) => "new ArrayList<>()".into(),
        Ty::Map { ordered, .. } => if *ordered {
            "new TreeMap<>()"
        } else {
            "new HashMap<>()"
        }
        .into(),
        Ty::Set { ordered, .. } => if *ordered {
            "new TreeSet<>()"
        } else {
            "new HashSet<>()"
        }
        .into(),
        Ty::Deque(_) => "new ArrayDeque<>()".into(),
        Ty::Heap(_) => "new PriorityQueue<>()".into(),
        Ty::Option(_) => "Optional.empty()".into(),
        Ty::Adt { java, .. } => format!("{java}.default_()"),
        Ty::Tuple(ts) if ts.len() == 2 => {
            format!(
                "Map.entry({}, {})",
                default_value(cx, &ts[0]),
                default_value(cx, &ts[1])
            )
        }
        _ => "null".into(),
    }
}

// Whether reordering evaluations of `e` cannot change side-effect order.
fn is_pure(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::Literal(_) | ast::Expr::PathExpr(_) => true,
        ast::Expr::ParenExpr(p) => p.expr().is_some_and(|x| is_pure(&x)),
        ast::Expr::FieldExpr(f) => f.expr().is_some_and(|x| is_pure(&x)),
        ast::Expr::RefExpr(r) => r.expr().is_some_and(|x| is_pure(&x)),
        ast::Expr::PrefixExpr(p) => p.expr().is_some_and(|x| is_pure(&x)),
        ast::Expr::CastExpr(c) => c.expr().is_some_and(|x| is_pure(&x)),
        ast::Expr::TupleExpr(t) => t.fields().all(|x| is_pure(&x)),
        _ => false,
    }
}

// ---- Representation contract shared by items.rs (declarations) and pat.rs (patterns) --------
//   enum with only unit variants  -> Java `enum E { A, B }`; value `E.A`; switch label `A`
//   any other enum                -> `sealed interface E` + one nested `record` per variant;
//                                    value `new E.V(..)`; pattern `E.V(var a, var b)`
//   record/tuple fields           -> named fields keep their name; tuple fields are `_0, _1, ...`
//   struct                        -> `static final class S` with the same field naming

pub fn enum_is_plain(cx: &Cx, e: ra_ap_hir::Enum) -> bool {
    e.variants(cx.db)
        .iter()
        .all(|v| v.kind(cx.db) == ra_ap_hir::StructKind::Unit)
}

// Java type name of a local ADT as seen from the current file (`Shape` or `Geometry.Shape`).
pub fn adt_java_name(cx: &Cx, adt: ra_ap_hir::Adt) -> String {
    let name = escape_keyword(adt.name(cx.db).as_str());
    let m = adt.module(cx.db);
    if m == cx.module {
        name
    } else {
        format!("{}.{name}", module_class_from(cx.db, cx.module, m))
    }
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
            if s.chars().all(|c| c.is_ascii_digit()) {
                format!("_{s}")
            } else {
                escape_keyword(s)
            }
        })
        .collect()
}

// Records that `TupleN` is needed; emitted once per file on the last line.
pub fn need_tuple(cx: &mut Cx, n: usize) {
    let params: Vec<String> = (0..n).map(|i| format!("T{i}")).collect();
    let fields: Vec<String> = (0..n).map(|i| format!("T{i} _{i}")).collect();
    let decl = format!(
        "record Tuple{n}<{}>({}) {{}}",
        params.join(", "),
        fields.join(", ")
    );
    if !cx.file_trailer.contains(&decl) {
        cx.file_trailer.insert(0, decl);
    }
}

// Kept for callers that only know the HIR type.
pub fn impl_owner_java(cx: &Cx, imp: Impl) -> String {
    let ty = imp.self_ty(cx.db);
    match cx.convert(&ty) {
        Ty::Adt { java, .. } => java,
        other => other.boxed(),
    }
}
