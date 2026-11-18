// Standard-trait support on user types: which local ADTs implement Iterator/Deref, and the
// receiver rewriting those impls need at call sites.

use ra_ap_syntax::{ast, ast::HasModuleItem, ast::HasName};

use super::{Cx, items};
use crate::ty::Ty;

#[derive(Clone, Default)]
pub struct TraitImpls {
    // `impl Iterator for T { type Item = I; }`
    pub iter_item: Option<Ty>,
    // `impl Deref for W { type Target = Vec<T>; }`
    pub deref_target: Option<Ty>,
}

// One pre-pass over the whole file so `main` can use impls declared after it.
pub fn collect(cx: &mut Cx, items: &[ast::Item]) {
    for item in items {
        match item {
            ast::Item::Impl(imp) => collect_impl(cx, imp),
            ast::Item::Module(m) => {
                if let Some(list) = m.item_list() {
                    let inner: Vec<ast::Item> = list.items().collect();
                    collect(cx, &inner);
                }
            }
            _ => {}
        }
    }
}

fn collect_impl(cx: &mut Cx, imp: &ast::Impl) {
    let Some(adt) = items::impl_self_adt(cx, imp) else {
        return;
    };
    let Some(tr) = items::impl_trait(cx, imp) else {
        return;
    };
    if tr.module(cx.db).krate(cx.db).origin(cx.db).is_local() {
        return;
    }
    let key = adt.name(cx.db).as_str().to_string();
    match tr.name(cx.db).as_str() {
        "Iterator" => {
            if let Some(ty) = impl_assoc_type(cx, imp, "Item") {
                cx.local_traits.entry(key).or_default().iter_item = Some(ty);
            }
        }
        "Deref" => {
            if let Some(ty) = impl_assoc_type(cx, imp, "Target") {
                cx.local_traits.entry(key).or_default().deref_target = Some(ty);
            }
        }
        _ => {}
    }
}

pub fn impl_assoc_type(cx: &mut Cx, imp: &ast::Impl, name: &str) -> Option<Ty> {
    let list = imp.assoc_item_list()?;
    for item in list.assoc_items() {
        let ast::AssocItem::TypeAlias(ta) = item else {
            continue;
        };
        if ta.name().is_some_and(|n| n.text() == name)
            && let Some(ty) = ta.ty()
        {
            return Some(cx.type_ref(&ty));
        }
    }
    None
}

// The `Iterator` item type of a local user type, if it implements Iterator.
pub fn iter_item(cx: &Cx, ty: &Ty) -> Option<Ty> {
    let Ty::Adt { name, args, .. } = ty.strip_refs() else {
        return None;
    };
    let item = cx.local_traits.get(name)?.iter_item.clone()?;
    Some(substitute(&item, args))
}

// The `Deref::Target` of a local user type, if it implements Deref.
pub fn deref_target(cx: &Cx, ty: &Ty) -> Option<Ty> {
    let Ty::Adt { name, args, .. } = ty.strip_refs() else {
        return None;
    };
    let target = cx.local_traits.get(name)?.deref_target.clone()?;
    Some(substitute(&target, args))
}

// Replaces type parameters of a stored target type with the use site's arguments.
fn substitute(ty: &Ty, args: &[Ty]) -> Ty {
    let mut names: Vec<String> = Vec::new();
    collect_params(ty, &mut names);
    let map: Vec<(String, Ty)> = names.into_iter().zip(args.iter().cloned()).collect();
    subst(ty, &map)
}

fn collect_params(ty: &Ty, out: &mut Vec<String>) {
    match ty {
        Ty::Param(p) => {
            if !out.contains(p) {
                out.push(p.clone());
            }
        }
        Ty::Ref { inner, .. } => collect_params(inner, out),
        _ => {}
    }
}

fn subst(ty: &Ty, map: &[(String, Ty)]) -> Ty {
    match ty {
        Ty::Param(p) => map
            .iter()
            .find(|(n, _)| n == p)
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| ty.clone()),
        Ty::Ref { mutable, inner } => Ty::Ref {
            mutable: *mutable,
            inner: Box::new(subst(inner, map)),
        },
        _ => ty.clone(),
    }
}
