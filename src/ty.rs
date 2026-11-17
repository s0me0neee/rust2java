#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Int {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

impl Int {
    pub fn from_name(s: &str) -> Option<Int> {
        Some(match s {
            "i8" => Int::I8,
            "i16" => Int::I16,
            "i32" => Int::I32,
            "i64" => Int::I64,
            "i128" => Int::I128,
            "isize" => Int::Isize,
            "u8" => Int::U8,
            "u16" => Int::U16,
            "u32" => Int::U32,
            "u64" => Int::U64,
            "u128" => Int::U128,
            "usize" => Int::Usize,
            _ => return None,
        })
    }

    pub fn is_long(self) -> bool {
        matches!(self, Int::I64 | Int::U64 | Int::I128 | Int::U128)
    }

    pub fn is_unsigned(self) -> bool {
        matches!(
            self,
            Int::U8 | Int::U16 | Int::U32 | Int::U64 | Int::U128 | Int::Usize
        )
    }

    pub fn java(self) -> &'static str {
        match self {
            Int::I8 => "byte",
            Int::I16 | Int::U16 => "short",
            Int::I64 | Int::U64 | Int::I128 | Int::U128 => "long",
            _ => "int",
        }
    }

    pub fn boxed(self) -> &'static str {
        match self.java() {
            "byte" => "Byte",
            "short" => "Short",
            "long" => "Long",
            _ => "Integer",
        }
    }
}

// Simplified view of a Rust type, as far as Java emission cares.
#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Unit,
    Never,
    Bool,
    Char,
    Int(Int),
    F32,
    F64,
    // `String`, `&str`, `Box<str>`
    Str,
    // `Vec<T>`, `[T; N]`, `[T]`
    Vec(Box<Ty>),
    Map {
        ordered: bool,
        k: Box<Ty>,
        v: Box<Ty>,
    },
    Set {
        ordered: bool,
        t: Box<Ty>,
    },
    Deque(Box<Ty>),
    Heap(Box<Ty>),
    Option(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    Tuple(Vec<Ty>),
    Ref {
        mutable: bool,
        inner: Box<Ty>,
    },
    // A type defined in the transpiled crate. `java` is its Java name as seen from the current file.
    Adt {
        name: String,
        java: String,
        args: Vec<Ty>,
    },
    // Anything implementing `Iterator`, with its item type.
    Iter(Box<Ty>),
    Fn {
        params: Vec<Ty>,
        ret: Box<Ty>,
    },
    Ordering,
    Param(String),
    // Unsupported or unknown; carries Rust's rendering for diagnostics.
    Other(String),
}

impl Ty {
    pub fn strip_refs(&self) -> &Ty {
        match self {
            Ty::Ref { inner, .. } => inner.strip_refs(),
            t => t,
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(
            self.strip_refs(),
            Ty::Bool | Ty::Char | Ty::Int(_) | Ty::F32 | Ty::F64
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self.strip_refs(), Ty::F32 | Ty::F64)
    }

    pub fn is_str(&self) -> bool {
        matches!(self.strip_refs(), Ty::Str)
    }

    pub fn is_unit(&self) -> bool {
        matches!(self.strip_refs(), Ty::Unit | Ty::Never)
    }

    // Java type in declaration position (primitives unboxed).
    pub fn java(&self) -> String {
        match self {
            Ty::Unit | Ty::Never => "void".into(),
            Ty::Bool => "boolean".into(),
            Ty::Char => "char".into(),
            Ty::Int(i) => i.java().into(),
            Ty::F32 => "float".into(),
            Ty::F64 => "double".into(),
            Ty::Ordering => "int".into(),
            Ty::Ref { inner, .. } => inner.java(),
            t => t.boxed(),
        }
    }

    // Java type in generic-argument position.
    pub fn boxed(&self) -> String {
        match self {
            Ty::Unit | Ty::Never => "Void".into(),
            Ty::Bool => "Boolean".into(),
            Ty::Char => "Character".into(),
            Ty::Int(i) => i.boxed().into(),
            Ty::F32 => "Float".into(),
            Ty::F64 => "Double".into(),
            Ty::Ordering => "Integer".into(),
            Ty::Str => "String".into(),
            Ty::Vec(t) => format!("List<{}>", t.boxed()),
            Ty::Map { ordered, k, v } => format!(
                "{}<{}, {}>",
                if *ordered { "TreeMap" } else { "HashMap" },
                k.boxed(),
                v.boxed()
            ),
            Ty::Set { ordered, t } => {
                format!(
                    "{}<{}>",
                    if *ordered { "TreeSet" } else { "HashSet" },
                    t.boxed()
                )
            }
            Ty::Deque(t) => format!("ArrayDeque<{}>", t.boxed()),
            Ty::Heap(t) => format!("PriorityQueue<{}>", t.boxed()),
            Ty::Option(t) => format!("Optional<{}>", t.boxed()),
            Ty::Result(t, e) => format!("Result<{}, {}>", t.boxed(), e.boxed()),
            Ty::Tuple(ts) if ts.is_empty() => "Void".into(),
            Ty::Tuple(ts) if ts.len() == 2 => {
                format!("Map.Entry<{}, {}>", ts[0].boxed(), ts[1].boxed())
            }
            Ty::Tuple(ts) => format!(
                "Tuple{}<{}>",
                ts.len(),
                ts.iter().map(Ty::boxed).collect::<Vec<_>>().join(", ")
            ),
            Ty::Ref { inner, .. } => inner.boxed(),
            Ty::Adt { java, args, .. } if args.is_empty() => java.clone(),
            Ty::Adt { java, args, .. } => format!(
                "{}<{}>",
                java,
                args.iter().map(Ty::boxed).collect::<Vec<_>>().join(", ")
            ),
            Ty::Iter(t) => format!("Stream<{}>", t.boxed()),
            Ty::Fn { params, ret } => fn_interface(params, ret),
            Ty::Param(p) => p.clone(),
            Ty::Other(_) => "Object".into(),
        }
    }

    // Whether a Java declaration can spell this type (otherwise use `var`).
    pub fn is_nameable(&self) -> bool {
        match self {
            Ty::Other(_) | Ty::Never => false,
            Ty::Vec(t)
            | Ty::Set { t, .. }
            | Ty::Deque(t)
            | Ty::Heap(t)
            | Ty::Option(t)
            | Ty::Iter(t) => t.is_nameable(),
            Ty::Map { k, v, .. } | Ty::Result(k, v) => k.is_nameable() && v.is_nameable(),
            Ty::Tuple(ts) => ts.iter().all(Ty::is_nameable),
            Ty::Ref { inner, .. } => inner.is_nameable(),
            Ty::Adt { args, .. } => args.iter().all(Ty::is_nameable),
            Ty::Fn { params, ret } => params.iter().all(Ty::is_nameable) && ret.is_nameable(),
            _ => true,
        }
    }
}

// Method that invokes a value of the functional interface chosen by `fn_interface`.
pub fn fn_call_method(params: usize, ret_unit: bool) -> &'static str {
    match (params, ret_unit) {
        (0, true) => "run",
        (0, false) => "get",
        (1 | 2, true) => "accept",
        _ => "apply",
    }
}

fn fn_interface(params: &[Ty], ret: &Ty) -> String {
    let b = |t: &Ty| t.boxed();
    match (params.len(), ret.is_unit()) {
        (0, true) => "Runnable".into(),
        (0, false) => format!("Supplier<{}>", b(ret)),
        (1, true) => format!("Consumer<{}>", b(&params[0])),
        (1, false) => format!("Function<{}, {}>", b(&params[0]), b(ret)),
        (2, true) => format!("BiConsumer<{}, {}>", b(&params[0]), b(&params[1])),
        (2, false) => format!(
            "BiFunction<{}, {}, {}>",
            b(&params[0]),
            b(&params[1]),
            b(ret)
        ),
        (n, _) => format!(
            "Fn{}<{}, {}>",
            n,
            params.iter().map(b).collect::<Vec<_>>().join(", "),
            b(ret)
        ),
    }
}
