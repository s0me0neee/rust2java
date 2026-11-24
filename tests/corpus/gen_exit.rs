use std::process;

#[derive(Debug)]
struct Config {
    width: u32,
    height: u32,
    title: String,
}

fn parse_args(args: &[&str]) -> Result<Config, String> {
    let mut width = None;
    let mut height = None;
    let mut title = String::from("untitled");
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-w" | "--width" => {
                i += 1;
                width = Some(args.get(i).ok_or("missing width")?.parse::<u32>().map_err(|e| format!("bad width: {e}"))?);
            }
            "-h" => {
                i += 1;
                height = args.get(i).and_then(|s| s.parse().ok());
            }
            t if t.starts_with("--title=") => title = t["--title=".len()..].to_string(),
            other => return Err(format!("unknown flag {other}")),
        }
        i += 1;
    }
    Ok(Config { width: width.ok_or("width is required")?, height: height.unwrap_or(10), title })
}

fn main() {
    for args in [vec!["-w", "80", "--title=hello"], vec!["-h", "5"], vec!["-w", "abc"], vec!["-w", "40", "-h", "x"]] {
        match parse_args(&args) {
            Ok(c) => println!("{c:?} area={}", c.width * c.height),
            Err(e) => println!("error: {e}"),
        }
    }
    let cfg = parse_args(&["--bogus"]).unwrap_or_else(|e| {
        println!("fatal: {e}");
        process::exit(2);
    });
    println!("unreachable {cfg:?}");
}
