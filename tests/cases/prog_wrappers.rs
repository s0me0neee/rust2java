// Generic wrappers: impl<T: Display> Display for Wrapper<T>, newtypes with Deref, From/Into
// conversions, TryFrom with errors, PartialOrd derive on structs, Ord via cmp chains, and Default.
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::ops::Deref;

struct Labeled<T> {
    label: &'static str,
    value: T,
}

impl<T: fmt::Display> fmt::Display for Labeled<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}=[{}]", self.label, self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Celsius(f64);
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Fahrenheit(f64);

impl From<Celsius> for Fahrenheit {
    fn from(c: Celsius) -> Self {
        Fahrenheit(c.0 * 9.0 / 5.0 + 32.0)
    }
}

#[derive(Debug, PartialEq)]
struct Even(u32);

impl TryFrom<i64> for Even {
    type Error = String;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        if v < 0 {
            Err(format!("{} is negative", v))
        } else if v % 2 != 0 {
            Err(format!("{} is odd", v))
        } else {
            Ok(Even(v as u32))
        }
    }
}

struct Names(Vec<String>);

impl Deref for Names {
    type Target = Vec<String>;
    fn deref(&self) -> &Vec<String> {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    pre: Option<String>,
}

impl Version {
    fn parse(s: &str) -> Option<Version> {
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (s, None),
        };
        let mut it = core.split('.').map(|p| p.parse::<u32>());
        let v = Version { major: it.next()?.ok()?, minor: it.next()?.ok()?, patch: it.next()?.ok()?, pre };
        if it.next().is_some() { None } else { Some(v) }
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        self.major
            .cmp(&o.major)
            .then(self.minor.cmp(&o.minor))
            .then(self.patch.cmp(&o.patch))
            .then_with(|| match (&self.pre, &o.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(p) = &self.pre {
            write!(f, "-{}", p)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct Settings {
    width: u32,
    title: String,
    tags: Vec<String>,
    scale: Option<f32>,
}

fn main() {
    println!("{} {} {}", Labeled { label: "n", value: 42 }, Labeled { label: "s", value: "hi" }, Labeled { label: "f", value: 2.5 });
    let temps = [Celsius(-40.0), Celsius(0.0), Celsius(36.6), Celsius(100.0)];
    let fs: Vec<Fahrenheit> = temps.iter().map(|&c| c.into()).collect();
    println!("{:?}", fs);
    println!("{} {}", Celsius(3.0) < Celsius(4.5), Fahrenheit::from(Celsius(10.0)) > Fahrenheit(49.0));
    for v in [4i64, 7, -2, 0] {
        let r: Result<Even, _> = Even::try_from(v);
        println!("{} -> {:?}", v, r);
    }
    let e: Result<Even, String> = 10i64.try_into();
    println!("{:?}", e);
    let names = Names(vec!["ann".into(), "bo".into(), "cy".into()]);
    println!("{} {} {:?} {}", names.len(), names[1], names.iter().map(|s| s.len()).max(), names.contains(&"cy".to_string()));
    let mut vs: Vec<Version> = ["1.2.3", "1.10.0", "1.2.3-beta", "0.9.12", "1.2", "2.0.0-rc1", "x.1.2", "1.2.3.4"]
        .iter()
        .filter_map(|s| Version::parse(s))
        .collect();
    vs.sort();
    println!("{}", vs.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" < "));
    println!("{:?}", vs.iter().max().map(|v| v.to_string()));
    println!("{:?}", Version::parse("3.1.4-pi").map(|v| v.cmp(&Version::parse("3.1.4").unwrap())));
    let s = Settings { width: 80, ..Default::default() };
    println!("{:?}", s);
    println!("{:?}", Settings { title: "t".into(), scale: Some(1.5), ..s });
}
