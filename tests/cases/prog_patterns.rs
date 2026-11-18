// Pattern matching: ranges, bindings @, guards, nested enums, slice patterns, tuple structs, let-else.
#[derive(Debug, Clone, PartialEq)]
enum Shape {
    Circle { r: f64 },
    Rect { w: f64, h: f64 },
    Tri(f64, f64, f64),
}

#[derive(Debug)]
enum Cmd {
    Move { dx: i32, dy: i32 },
    Say(String),
    Batch(Vec<Cmd>),
    Quit,
}

struct Meters(f64);

fn classify(n: i64) -> &'static str {
    match n {
        i64::MIN..=-1 => "negative",
        0 => "zero",
        1..=9 => "digit",
        10 | 20 | 30 => "round",
        x if x % 2 == 0 => "even",
        _ => "odd",
    }
}

fn describe(s: &Shape) -> String {
    match s {
        Shape::Circle { r } if *r <= 0.0 => "degenerate circle".to_string(),
        Shape::Circle { r } => format!("circle area {:.2}", std::f64::consts::PI * r * r),
        Shape::Rect { w, h } if w == h => format!("square {}", w),
        Shape::Rect { w, .. } if *w > 10.0 => "wide rect".into(),
        Shape::Rect { w, h } => format!("rect {}x{}", w, h),
        &Shape::Tri(a, b, c) => {
            let s = (a + b + c) / 2.0;
            format!("triangle area {:.3}", (s * (s - a) * (s - b) * (s - c)).sqrt())
        }
    }
}

fn run(cmd: &Cmd, pos: &mut (i32, i32), log: &mut Vec<String>) -> bool {
    match cmd {
        Cmd::Move { dx: 0, dy: 0 } => log.push("noop".into()),
        Cmd::Move { dx, dy } => {
            pos.0 += dx;
            pos.1 += dy;
        }
        Cmd::Say(msg) if msg.is_empty() => log.push("...".into()),
        Cmd::Say(msg) => log.push(msg.clone()),
        Cmd::Batch(cmds) => {
            for c in cmds {
                if !run(c, pos, log) {
                    return false;
                }
            }
        }
        Cmd::Quit => return false,
    }
    true
}

fn sum_ends(xs: &[i32]) -> i32 {
    match xs {
        [] => 0,
        [x] => *x,
        [first, .., last] => first + last,
    }
}

fn head_tail(xs: &[&str]) -> String {
    match xs {
        [] => "empty".into(),
        [one] => format!("just {}", one),
        [first, rest @ ..] => format!("{} then {} more", first, rest.len()),
    }
}

fn parse_point(s: &str) -> Option<(i32, i32)> {
    let Some((a, b)) = s.split_once(',') else {
        return None;
    };
    let (Ok(x), Ok(y)) = (a.trim().parse::<i32>(), b.trim().parse::<i32>()) else {
        return None;
    };
    Some((x, y))
}

fn main() {
    for n in [-5, 0, 7, 20, 44, 51] {
        print!("{}:{} ", n, classify(n));
    }
    println!();
    let shapes = [
        Shape::Circle { r: 1.5 },
        Shape::Circle { r: 0.0 },
        Shape::Rect { w: 3.0, h: 3.0 },
        Shape::Rect { w: 12.0, h: 1.0 },
        Shape::Rect { w: 2.0, h: 5.0 },
        Shape::Tri(3.0, 4.0, 5.0),
    ];
    for s in &shapes {
        println!("{}", describe(s));
    }
    let squares = shapes.iter().filter(|s| matches!(s, Shape::Rect { w, h } if w == h)).count();
    println!("squares={} first circle={:?}", squares, shapes.iter().position(|s| matches!(s, Shape::Circle { .. })));

    let program = Cmd::Batch(vec![
        Cmd::Move { dx: 2, dy: -1 },
        Cmd::Say("hi".to_string()),
        Cmd::Move { dx: 0, dy: 0 },
        Cmd::Batch(vec![Cmd::Say(String::new()), Cmd::Move { dx: -5, dy: 10 }]),
        Cmd::Quit,
        Cmd::Say("never".into()),
    ]);
    let mut pos = (0, 0);
    let mut log = vec![];
    let finished = run(&program, &mut pos, &mut log);
    println!("{} {:?} {:?}", finished, pos, log);

    println!("{} {} {} {}", sum_ends(&[]), sum_ends(&[7]), sum_ends(&[1, 2, 3, 4]), head_tail(&["a", "b", "c"]));
    println!("{} {}", head_tail(&[]), head_tail(&["z"]));
    for s in ["3,4", " -1 , 8", "x,1", "12"] {
        println!("{:?} -> {:?}", s, parse_point(s));
    }
    let m = Meters(12.5);
    let Meters(inner) = m;
    let msg = match inner as u32 {
        n @ 0..=9 => format!("small {}", n),
        n @ (10 | 11 | 12) => format!("dozenish {}", n),
        n => format!("big {}", n),
    };
    println!("{} {}", inner, msg);
    let pair = (3, "three");
    let text = match pair {
        (0, _) => "zero".to_string(),
        (n, name) if n > 2 => format!("{}={}", name, n),
        (_, name) => name.to_string(),
    };
    println!("{}", text);
    let nested = Some(Some(5));
    if let Some(Some(x @ 1..=9)) = nested {
        println!("nested digit {}", x);
    }
    let opt: Option<&Shape> = shapes.get(5);
    if let Some(Shape::Tri(a, ..)) = opt {
        println!("tri side {}", a);
    }
}
