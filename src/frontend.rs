// The only module that talks to rust-analyzer's project loading and type representation.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use ra_ap_hir::{Adt, Crate, HirDisplay, Module, ModuleSource, Semantics};
use ra_ap_ide_db::RootDatabase;
use ra_ap_ide_db::base_db::{EditionedFileId, LangCrateOrigin};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::{CargoConfig, RustLibSource};
use ra_ap_vfs::Vfs;

use crate::ty::{Int, Ty};

pub struct Project {
    pub db: RootDatabase,
    pub vfs: Vfs,
    // Keeps a generated wrapper project alive for single-file inputs.
    _tmp: Option<tempfile::TempDir>,
}

pub struct Unit {
    pub krate: Crate,
    pub package: String,
    pub module: Module,
    pub file_id: EditionedFileId,
    pub path: PathBuf,
    pub class_name: String,
    pub is_root: bool,
}

pub fn load(inputs: &[PathBuf]) -> Result<Project> {
    let (root, tmp) = if inputs.len() == 1 && inputs[0].is_dir() {
        (inputs[0].clone(), None)
    } else {
        let tmp = wrap_files(inputs)?;
        (tmp.path().to_path_buf(), Some(tmp))
    };
    let cargo_config = CargoConfig { sysroot: Some(RustLibSource::Discover), ..CargoConfig::default() };
    let load_config = LoadCargoConfig {
        load_out_dirs_from_check: false,
        with_proc_macro_server: ProcMacroServerChoice::None,
        prefill_caches: false,
        num_worker_threads: 1,
        proc_macro_processes: 1,
    };
    let (db, vfs, _) = load_workspace_at(&root, &cargo_config, &load_config, &|_| {})
        .with_context(|| format!("loading {}", root.display()))?;
    Ok(Project { db, vfs, _tmp: tmp })
}

// Single .rs files become [[bin]] targets of one generated cargo project, so std is loaded once.
fn wrap_files(files: &[PathBuf]) -> Result<tempfile::TempDir> {
    let tmp = tempfile::tempdir()?;
    let mut manifest = String::from(
        "[package]\nname = \"rust2java_inputs\"\nversion = \"0.0.0\"\nedition = \"2024\"\nautobins = false\n\n",
    );
    for f in files {
        if f.extension().is_none_or(|e| e != "rs") {
            bail!("{}: expected a cargo project directory or .rs files", f.display());
        }
        let abs = std::fs::canonicalize(f).with_context(|| format!("reading {}", f.display()))?;
        let stem = abs.file_stem().unwrap().to_string_lossy().replace('-', "_");
        manifest.push_str(&format!("[[bin]]\nname = \"{stem}\"\npath = {:?}\n\n", abs.display().to_string()));
    }
    std::fs::write(tmp.path().join("Cargo.toml"), manifest)?;
    Ok(tmp)
}

impl Project {
    pub fn units(&self) -> Vec<Unit> {
        let db = &self.db;
        let mut units = Vec::new();
        for krate in Crate::all(db) {
            if !krate.origin(db).is_local() {
                continue;
            }
            let package = krate
                .display_name(db)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "main".into())
                .replace('-', "_");
            for module in krate.modules(db) {
                let Some(file_id) = module.definition_source_file_id(db).file_id() else { continue };
                // Inline `mod x { .. }` shares the parent file; it is lowered as a nested class.
                if !matches!(module.definition_source(db).value, ModuleSource::SourceFile(_)) {
                    continue;
                }
                let path = self.vfs.file_path(file_id.file_id(db)).to_string();
                let is_root = module.is_crate_root(db);
                let class_name = if is_root {
                    "Main".to_string()
                } else {
                    let name = module.name(db).map(|n| n.as_str().to_string()).unwrap_or_default();
                    pascal_case(&name)
                };
                units.push(Unit {
                    krate,
                    package: package.clone(),
                    module,
                    file_id,
                    path: PathBuf::from(path),
                    class_name,
                    is_root,
                });
            }
        }
        units
    }
}

pub fn pascal_case(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

// Fully qualified Java class path of a module, e.g. `Main`, `Geometry`, `Geometry.Inner`.
pub fn module_java_path(db: &RootDatabase, module: Module) -> String {
    if module.is_crate_root(db) {
        return "Main".into();
    }
    let name = pascal_case(module.name(db).map(|n| n.as_str().to_string()).unwrap_or_default().as_str());
    if matches!(module.definition_source(db).value, ModuleSource::SourceFile(_)) {
        name
    } else {
        match module.parent(db) {
            Some(parent) => format!("{}.{}", module_java_path(db, parent), name),
            None => name,
        }
    }
}

// First module whose definition is a whole file.
pub fn top_module(db: &RootDatabase, mut module: Module) -> Module {
    while !matches!(module.definition_source(db).value, ModuleSource::SourceFile(_)) {
        match module.parent(db) {
            Some(p) => module = p,
            None => break,
        }
    }
    module
}

#[allow(dead_code)]
pub fn module_class_name(db: &RootDatabase, module: Module) -> String {
    module_java_path(db, module)
}

// Module class name as seen from `from`: relative inside one file, qualified across files.
pub fn module_class_from(db: &RootDatabase, from: Module, module: Module) -> String {
    if from == module {
        return module_java_path(db, module);
    }
    if top_module(db, module) == from {
        let full = module_java_path(db, module);
        let top = module_java_path(db, from);
        if let Some(rel) = full.strip_prefix(&format!("{top}.")) {
            return rel.to_string();
        }
    }
    module_java_path(db, module)
}

// Java name of a trait as seen from module `from`.
pub fn trait_java_name(db: &RootDatabase, from: Module, tr: ra_ap_hir::Trait) -> String {
    let name = tr.name(db).as_str().to_string();
    if tr.module(db) == from {
        name
    } else {
        format!("{}.{}", module_class_from(db, from, tr.module(db)), name)
    }
}

// Converts rust-analyzer's type into our simplified model. `from` is the module being emitted,
// used to qualify user types defined in other modules.
pub fn convert(sema: &Semantics<'_, RootDatabase>, from: Module, ty: &ra_ap_hir::Type<'_>) -> Ty {
    let db = sema.db;
    if ty.is_unit() {
        return Ty::Unit;
    }
    if ty.is_never() {
        return Ty::Never;
    }
    if ty.is_reference() {
        let inner = ty.as_reference().map(|(t, _)| convert(sema, from, &t)).unwrap_or(Ty::Other("&?".into()));
        if inner == Ty::Str && !ty.is_mutable_reference() {
            return Ty::Str;
        }
        return Ty::Ref { mutable: ty.is_mutable_reference(), inner: Box::new(inner) };
    }
    if ty.is_bool() {
        return Ty::Bool;
    }
    if ty.is_char() {
        return Ty::Char;
    }
    if ty.is_str() {
        return Ty::Str;
    }
    if let Some(b) = ty.as_builtin() {
        let name = b.name();
        let name = name.as_str();
        if let Some(i) = Int::from_name(name) {
            return Ty::Int(i);
        }
        match name {
            "f32" => return Ty::F32,
            "f64" => return Ty::F64,
            _ => {}
        }
    }
    if let Some(elem) = ty.as_slice() {
        return Ty::Vec(Box::new(convert(sema, from, &elem)));
    }
    if let Some((elem, _)) = ty.as_array(db) {
        return Ty::Vec(Box::new(convert(sema, from, &elem)));
    }
    if ty.is_tuple() {
        return Ty::Tuple(ty.tuple_fields(db).iter().map(|t| convert(sema, from, t)).collect());
    }
    if let Some(p) = ty.as_type_param(db) {
        return Ty::Param(p.name(db).as_str().to_string());
    }
    // `dyn Trait` / `impl Trait` of a crate trait become its interface; std traits (Fn*, Iterator)
    // fall through to the callable / iterator mappings below.
    if let Some(tr) = ty.as_dyn_trait().or_else(|| ty.as_impl_traits(db).and_then(|mut it| it.next()))
        && tr.module(db).krate(db).origin(db).is_local()
    {
        return Ty::Adt { name: tr.name(db).as_str().to_string(), java: trait_java_name(db, from, tr), args: vec![] };
    }
    let args: Vec<Ty> = ty.type_arguments().map(|t| convert(sema, from, &t)).collect();
    let arg = |i: usize| Box::new(args.get(i).cloned().unwrap_or(Ty::Other("?".into())));
    if let Some(adt) = ty.as_adt() {
        let module = adt.module(db);
        let krate = module.krate(db);
        let name = adt.name(db).as_str().to_string();
        if krate.origin(db).is_local() {
            let java = if module == from { name.clone() } else { format!("{}.{}", module_class_from(sema.db, from, module), name) };
            let args = match adt {
                Adt::Struct(_) | Adt::Enum(_) | Adt::Union(_) => args.clone(),
            };
            return Ty::Adt { name, java, args };
        }
        if matches!(
            krate.origin(db),
            ra_ap_ide_db::base_db::CrateOrigin::Lang(
                LangCrateOrigin::Std | LangCrateOrigin::Core | LangCrateOrigin::Alloc
            )
        ) {
            match name.as_str() {
                "String" => return Ty::Str,
                "Vec" => return Ty::Vec(arg(0)),
                "HashMap" => return Ty::Map { ordered: false, k: arg(0), v: arg(1) },
                "BTreeMap" => return Ty::Map { ordered: true, k: arg(0), v: arg(1) },
                "HashSet" => return Ty::Set { ordered: false, t: arg(0) },
                "BTreeSet" => return Ty::Set { ordered: true, t: arg(0) },
                "VecDeque" => return Ty::Deque(arg(0)),
                "BinaryHeap" => return Ty::Heap(arg(0)),
                "Option" => return Ty::Option(arg(0)),
                "Result" => return Ty::Result(arg(0), arg(1)),
                "Box" | "Rc" | "Arc" | "RefCell" | "Cell" | "Reverse" | "Wrapping" => return *arg(0),
                "Ordering" => return Ty::Ordering,
                _ => {}
            }
        }
    }
    if let Some(c) = ty.as_callable(db) {
        let params = c.params().iter().map(|p| convert(sema, from, p.ty())).collect();
        let ret = convert(sema, from, &c.return_type());
        return Ty::Fn { params, ret: Box::new(ret) };
    }
    if ty.clone().impls_iterator(db)
        && let Some(item) = ty.clone().iterator_item(db)
    {
        return Ty::Iter(Box::new(convert(sema, from, &item)));
    }
    Ty::Other(ty.display(db, from.krate(db).to_display_target(db)).to_string())
}
