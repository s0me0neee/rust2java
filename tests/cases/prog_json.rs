// A small JSON parser over bytes with a recursive enum, BTreeMap objects, error positions,
// and a pretty printer that writes into a String with write!.
use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

type PResult<T> = Result<T, String>;

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Parser { src: s.as_bytes(), pos: 0 }
    }

    fn ws(&mut self) {
        while self.pos < self.src.len() && (self.src[self.pos] as char).is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, b: u8) -> PResult<()> {
        self.ws();
        if self.src.get(self.pos) == Some(&b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at {}", b as char, self.pos))
        }
    }

    fn value(&mut self) -> PResult<Json> {
        self.ws();
        match self.src.get(self.pos).copied() {
            None => Err("unexpected end".to_string()),
            Some(b'n') => self.lit("null", Json::Null),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.ws();
                if self.src.get(self.pos) == Some(&b']') {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    self.ws();
                    match self.src.get(self.pos) {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            break;
                        }
                        _ => return Err(format!("bad array at {}", self.pos)),
                    }
                }
                Ok(Json::Arr(items))
            }
            Some(b'{') => {
                self.pos += 1;
                let mut obj = BTreeMap::new();
                loop {
                    self.ws();
                    if self.src.get(self.pos) == Some(&b'}') {
                        self.pos += 1;
                        break;
                    }
                    let k = self.string()?;
                    self.expect(b':')?;
                    let v = self.value()?;
                    obj.insert(k, v);
                    self.ws();
                    if self.src.get(self.pos) == Some(&b',') {
                        self.pos += 1;
                    }
                }
                Ok(Json::Obj(obj))
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => {
                let start = self.pos;
                self.pos += 1;
                while self.pos < self.src.len() && matches!(self.src[self.pos], b'0'..=b'9' | b'.' | b'e' | b'-') {
                    self.pos += 1;
                }
                let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
                text.parse::<f64>().map(Json::Num).map_err(|e| format!("{} for {:?}", e, text))
            }
            Some(c) => Err(format!("unexpected '{}' at {}", c as char, self.pos)),
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> PResult<Json> {
        if self.src[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(v)
        } else {
            Err(format!("bad literal at {}", self.pos))
        }
    }

    fn string(&mut self) -> PResult<String> {
        self.ws();
        if self.src.get(self.pos) != Some(&b'"') {
            return Err(format!("expected string at {}", self.pos));
        }
        self.pos += 1;
        let mut out = String::new();
        while let Some(&b) = self.src.get(self.pos) {
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.src[self.pos];
                    self.pos += 1;
                    out.push(match e {
                        b'n' => '\n',
                        b't' => '\t',
                        other => other as char,
                    });
                }
                _ => out.push(b as char),
            }
        }
        Err("unterminated string".into())
    }
}

fn render(j: &Json, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match j {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => write!(out, "{}", b).unwrap(),
        Json::Num(n) => write!(out, "{}", n).unwrap(),
        Json::Str(s) => write!(out, "{:?}", s).unwrap(),
        Json::Arr(items) if items.is_empty() => out.push_str("[]"),
        Json::Arr(items) => {
            out.push_str("[\n");
            for (i, it) in items.iter().enumerate() {
                out.push_str(&pad);
                out.push_str("  ");
                render(it, indent + 1, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            write!(out, "{}]", pad).unwrap();
        }
        Json::Obj(map) => {
            out.push_str("{\n");
            let n = map.len();
            for (i, (k, v)) in map.iter().enumerate() {
                write!(out, "{}  {:?}: ", pad, k).unwrap();
                render(v, indent + 1, out);
                out.push_str(if i + 1 < n { ",\n" } else { "\n" });
            }
            out.push_str(&pad);
            out.push('}');
        }
    }
}

fn depth(j: &Json) -> usize {
    match j {
        Json::Arr(v) => 1 + v.iter().map(depth).max().unwrap_or(0),
        Json::Obj(m) => 1 + m.values().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

fn main() {
    let text = r#"{"name": "demo", "tags": ["a", "b\n"], "n": -12.5e1, "ok": true, "nested": {"x": [1, [2, []]], "y": null}}"#;
    match Parser::new(text).value() {
        Ok(j) => {
            let mut s = String::new();
            render(&j, 0, &mut s);
            println!("{}", s);
            println!("depth {}", depth(&j));
            if let Json::Obj(m) = &j {
                println!("keys {:?}", m.keys().collect::<Vec<_>>());
                println!("{:?}", m.get("tags"));
            }
        }
        Err(e) => println!("error {}", e),
    }
    for bad in ["[1, 2", "{\"a\" 1}", "tru", "\"open", "[1 2]", "-"] {
        println!("{:<10} -> {:?}", bad, Parser::new(bad).value());
    }
    println!("{}", Parser::new("[null, false]").value() == Ok(Json::Arr(vec![Json::Null, Json::Bool(false)])));
}
