// `From`/`Into`, `TryFrom`/`try_into`, `FromStr` and `.parse::<T>()` for user types.
use std::str::FromStr;

struct Celsius(f64);
struct Fahrenheit(f64);
impl From<Celsius> for Fahrenheit {
    fn from(c: Celsius) -> Self { Fahrenheit(c.0 * 9.0 / 5.0 + 32.0) }
}
impl From<f64> for Celsius {
    fn from(v: f64) -> Self { Celsius(v) }
}

struct Meters(i32);
impl FromStr for Meters {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<i32>() {
            Ok(v) => Ok(Meters(v)),
            Err(_) => Err(format!("bad: {s}")),
        }
    }
}

struct Port(u16);
impl TryFrom<i32> for Port {
    type Error = String;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        if v >= 0 && v <= 65535 { Ok(Port(v as u16)) } else { Err("out of range".to_string()) }
    }
}

fn main() {
    let f: Fahrenheit = Celsius(100.0).into();
    println!("{}", f.0);
    let f2 = Fahrenheit::from(Celsius(0.0));
    println!("{}", f2.0);
    let c: Celsius = 21.5.into();
    println!("{}", c.0);
    let m: Meters = "42".parse().unwrap();
    println!("{}", m.0);
    let m2 = Meters::from_str("7").unwrap();
    println!("{}", m2.0);
    match "x".parse::<Meters>() {
        Ok(v) => println!("{}", v.0),
        Err(e) => println!("{}", e),
    }
    let p: Result<Port, String> = 8080i32.try_into();
    println!("{}", p.unwrap().0);
    let p2 = Port::try_from(70000);
    println!("{}", p2.is_err());
}
