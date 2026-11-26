# Writes tests/core_api.txt: the stable "everyday core" API that tests/core_coverage.rs requires
# some test case to call. Reads the local std docs: `rustup component add rust-docs`, then
#   python3 tests/core_api.py > tests/core_api.txt
# Scope: primitives, Option/Result, iterators, cmp/convert/clone traits, cells, ranges, Duration,
# and the iter/mem/cmp/char/convert functions. Unsafe and raw-memory items have no Java meaning
# and are left out (LOW_LEVEL). Integer widths fold into iN/uN, floats into fN.
import os
import re
import subprocess

SYSROOT = subprocess.run(["rustc", "+stable", "--print", "sysroot"], capture_output=True, text=True).stdout.strip()
DOC = os.path.join(SYSROOT, "share/doc/rust/html/std/")

# Key owner -> doc page; owners are what `R2J_STD_CALLS` writes (primitive, type or trait name).
PAGES = {
    "iN": "primitive.i64.html",
    "uN": "primitive.u64.html",
    "fN": "primitive.f64.html",
    "char": "primitive.char.html",
    "bool": "primitive.bool.html",
    "str": "primitive.str.html",
    "slice": "primitive.slice.html",
    "array": "primitive.array.html",
    "Option": "option/enum.Option.html",
    "Result": "result/enum.Result.html",
    "Iterator": "iter/trait.Iterator.html",
    "DoubleEndedIterator": "iter/trait.DoubleEndedIterator.html",
    "ExactSizeIterator": "iter/trait.ExactSizeIterator.html",
    "IntoIterator": "iter/trait.IntoIterator.html",
    "FromIterator": "iter/trait.FromIterator.html",
    "Extend": "iter/trait.Extend.html",
    "Sum": "iter/trait.Sum.html",
    "Product": "iter/trait.Product.html",
    "Ordering": "cmp/enum.Ordering.html",
    "PartialEq": "cmp/trait.PartialEq.html",
    "PartialOrd": "cmp/trait.PartialOrd.html",
    "Ord": "cmp/trait.Ord.html",
    "Clone": "clone/trait.Clone.html",
    "Default": "default/trait.Default.html",
    "From": "convert/trait.From.html",
    "Into": "convert/trait.Into.html",
    "TryFrom": "convert/trait.TryFrom.html",
    "TryInto": "convert/trait.TryInto.html",
    "AsRef": "convert/trait.AsRef.html",
    "AsMut": "convert/trait.AsMut.html",
    "FromStr": "str/trait.FromStr.html",
    "Cell": "cell/struct.Cell.html",
    "RefCell": "cell/struct.RefCell.html",
    "OnceCell": "cell/struct.OnceCell.html",
    "Range": "ops/struct.Range.html",
    "RangeInclusive": "ops/struct.RangeInclusive.html",
    "Duration": "time/struct.Duration.html",
    "Wrapping": "num/struct.Wrapping.html",
}
FN_MODULES = ["iter", "mem", "cmp", "char", "convert"]

LOW_LEVEL = re.compile(
    r"allocator|_in$|raw|ptr|uninit|zeroed|non_null|^unchecked_|_unchecked|^leak$|pin|spare_capacity"
    r"|set_len|assume_init|from_parts|into_parts|downcast|^as_mut_ptr$|^transmute|^forget_unsized"
    r"|^needs_drop$|^variant_count$|^undo_leak$|^conjure_zst$|^as_bytes_mut$|^substr_range$"
    r"|^subslice_range$|^element_offset$|^align_to|^write_clone_of_slice$|^write_copy_of_slice$"
    r"|^try_borrow_unguarded$|^as_array_of_cells$|^from_mut$|^as_slice_of_cells$"
)


def section(html, is_trait):
    # A trait page's own methods; a type page's inherent impls (not trait or Deref methods).
    if is_trait:
        cut = [html.find(m) for m in ['id="implementors"', 'id="foreign-impls"', 'id="dyn-compatibility"']]
        cut = [c for c in cut if c > 0]
        return html[: min(cut)] if cut else html
    if 'id="implementations-list"' not in html:
        return ""
    start = html.index('id="implementations-list"')
    ends = [html.find(m, start) for m in ['id="synthetic-implementations', 'id="trait-implementations',
                                           'id="blanket-implementations', 'id="deref-methods']]
    ends = [e for e in ends if e > 0]
    return html[start: min(ends) if ends else len(html)]


def methods(page, is_trait):
    html = section(open(DOC + page, encoding="utf-8").read(), is_trait)
    names = set()
    parts = re.split(r'id="(?:ty)?method\.([a-z_0-9]+)"', html)
    for i in range(1, len(parts), 2):
        body = parts[i + 1][:5000]
        nxt = re.search(r'id="(?:ty)?method\.', body)
        if nxt:
            body = body[: nxt.start()]
        if "nightly-only experimental" in body or "Deprecated" in body[:800]:
            continue
        if not LOW_LEVEL.search(parts[i]):
            names.add(parts[i])
    return names


def free_fns(module):
    out = set()
    for f in os.listdir(DOC + module):
        m = re.fullmatch(r"fn\.([a-z_0-9]+)\.html", f)
        if not m or LOW_LEVEL.search(m.group(1)):
            continue
        html = open(os.path.join(DOC, module, f), encoding="utf-8").read()
        main = html[html.find('id="main-content"'):][:6000]
        if "nightly-only experimental" in main or "Deprecated" in main[:1500]:
            continue
        out.add(m.group(1))
    return out


keys = set()
for owner, page in PAGES.items():
    keys |= {f"{owner}::{n}" for n in methods(page, "/trait." in page)}
for module in FN_MODULES:
    keys |= {f"{module}::{n}" for n in free_fns(module)}
print("\n".join(sorted(keys)))
