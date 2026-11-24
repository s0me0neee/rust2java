use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    fn render(&self, indent: usize, out: &mut String) {
        let pad = "  ".repeat(indent);
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Num(n) => out.push_str(&n.to_string()),
            Value::Str(s) => {
                out.push('"');
                out.push_str(&s.replace('"', "\\\""));
                out.push('"');
            }
            Value::List(items) if items.is_empty() => out.push_str("[]"),
            Value::List(items) => {
                out.push_str("[\n");
                for (i, v) in items.iter().enumerate() {
                    out.push_str(&pad);
                    out.push_str("  ");
                    v.render(indent + 1, out);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad);
                out.push(']');
            }
            Value::Obj(fields) => {
                out.push_str("{\n");
                for (i, (k, v)) in fields.iter().enumerate() {
                    out.push_str(&format!("{pad}  \"{k}\": "));
                    v.render(indent + 1, out);
                    if i + 1 < fields.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push('}');
            }
        }
    }

    fn depth(&self) -> usize {
        match self {
            Value::List(v) => 1 + v.iter().map(Value::depth).max().unwrap_or(0),
            Value::Obj(f) => 1 + f.iter().map(|(_, v)| v.depth()).max().unwrap_or(0),
            _ => 0,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.render(0, &mut s);
        f.write_str(&s)
    }
}

fn main() {
    let v = Value::Obj(vec![
        ("name".into(), Value::Str("Widget \"Pro\"".into())),
        ("price".into(), Value::Num(19.5)),
        ("tags".into(), Value::List(vec![Value::Str("a".into()), Value::Bool(true), Value::Null])),
        ("dims".into(), Value::Obj(vec![("w".into(), Value::Num(3.0)), ("h".into(), Value::List(vec![]))])),
    ]);
    println!("{v}");
    println!("depth {}", v.depth());
    let copy = v.clone();
    println!("{}", copy == v);
}
