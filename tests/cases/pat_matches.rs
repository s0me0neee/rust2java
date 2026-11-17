#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Circle(f64),
    Rect { w: f64, h: f64 },
    Dot,
}

fn main() {
    let n = 7;
    println!("{}", matches!(n, 1..=9));
    println!("{}", matches!(n, 0 | 2 | 4));
    let c = 'q';
    println!("{}", matches!(c, 'a'..='z' | 'A'..='Z'));
    let o: Option<i32> = Some(12);
    println!("{}", matches!(o, Some(x) if x > 10));
    println!("{}", matches!(o, Some(1..=5)));
    println!("{}", matches!(o, None));
    let r: Result<i32, String> = Err("bad".to_string());
    println!("{}", matches!(r, Err(_)));
    let shapes = vec![Shape::Circle(1.0), Shape::Rect { w: 2.0, h: 3.0 }, Shape::Dot];
    for s in &shapes {
        let round = matches!(s, Shape::Circle(_) | Shape::Dot);
        let wide = matches!(s, Shape::Rect { w, h } if w < h);
        println!("{:?} {} {}", s, round, wide);
    }
    let pair = (3, "x");
    println!("{}", matches!(pair, (3, "x")));
    let count = shapes.iter().filter(|s| matches!(s, Shape::Rect { .. })).count();
    println!("{}", count);
}
