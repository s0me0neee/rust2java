// Cell / RefCell / OnceCell, Range / RangeInclusive, mem functions, array methods, Wrapping.
use std::cell::{Cell, OnceCell, RefCell};
use std::mem;
use std::num::Wrapping;
use std::ops::RangeInclusive;

#[derive(Debug)]
enum Shape {
    Dot,
    Line(i32),
    Box { w: i32, h: i32 },
}

fn main() {
    // Cell
    let mut c = Cell::new(5);
    *c.get_mut() += 1;
    println!("{} {}", c.get(), c.into_inner());

    // RefCell
    let mut r = RefCell::new(vec![1, 2]);
    r.get_mut().push(3);
    let old = r.replace_with(|v| v.iter().map(|x| x * 10).collect());
    println!("{:?} {:?}", old, r.borrow());
    let other = RefCell::new(vec![9]);
    r.swap(&other);
    println!("{:?} {:?}", r.borrow(), other.borrow());
    // Only uncontended borrows: Java keeps no borrow flags, so a conflicting one would succeed.
    println!("{} {:?}", r.try_borrow().is_ok(), r.try_borrow().map(|v| v.len()));
    if let Ok(mut v) = r.try_borrow_mut() {
        v.push(7);
    }
    println!("{:?}", r.borrow());

    // OnceCell
    let mut once: OnceCell<String> = OnceCell::new();
    println!("{:?}", once.get());
    let first = once.get_or_init(|| "first".to_string()).clone();
    println!("{} {:?} {:?}", first, once.set("second".to_string()), once.get());
    if let Some(s) = once.get_mut() {
        s.push('!');
    }
    println!("{:?} {:?}", once.take(), once.get());
    once.set("third".to_string()).unwrap();
    println!("{:?}", once.into_inner());
    let filled = OnceCell::from(3);
    println!("{:?}", filled.into_inner());

    // Range / RangeInclusive
    let r1 = 3..3;
    let r2 = 2..5;
    println!("{} {} {}", r1.is_empty(), r2.is_empty(), (5..1).is_empty());
    let ri = RangeInclusive::new(1, 4);
    println!("{} {} {} {}", ri.contains(&4), ri.contains(&5), ri.start(), ri.end());
    println!("{} {}", ri.is_empty(), (4..=1).is_empty());
    let (lo, hi) = (10..=20).into_inner();
    println!("{lo} {hi} {}", (1..=3).sum::<i32>());

    // mem
    println!("{} {} {} {}", mem::size_of::<i32>(), mem::size_of::<u8>(), mem::size_of::<f64>(), mem::size_of::<char>());
    println!("{} {} {}", mem::align_of::<i64>(), mem::align_of::<u16>(), mem::size_of_val(&7u16));
    println!("{} {}", mem::align_of_val(&1.5f32), mem::size_of_val(&[1u32, 2, 3]));
    let shapes = [Shape::Dot, Shape::Line(1), Shape::Line(2), Shape::Box { w: 1, h: 2 }];
    println!(
        "{} {} {}",
        mem::discriminant(&shapes[1]) == mem::discriminant(&shapes[2]),
        mem::discriminant(&shapes[0]) == mem::discriminant(&shapes[3]),
        mem::discriminant(&shapes[3]) == mem::discriminant(&Shape::Box { w: 0, h: 0 })
    );
    println!("{:?}", shapes);
    let gone = vec![1, 2, 3];
    mem::drop(gone);
    let kept = String::from("kept");
    mem::forget(kept);

    // arrays
    let arr = [3, 1, 2];
    let doubled = arr.map(|x| x * 2);
    println!("{:?} {:?} {}", doubled, arr.as_slice(), arr.as_slice().len());
    let words = ["a".to_string(), "bc".to_string()];
    let lens = words.each_ref().map(|s| s.len());
    println!("{:?}", lens);
    let mut grid = [vec![1], vec![2]];
    for v in grid.each_mut() {
        v.push(0);
    }
    grid.as_mut_slice().reverse();
    println!("{:?}", grid);

    // Wrapping
    let w = Wrapping(0b1011u8);
    println!("{} {}", w.reverse_bits(), (Wrapping(250u8) + Wrapping(10)).0);
    let mut a = Wrapping(250u8);
    a += Wrapping(10);
    let mut b = Wrapping(200u8);
    b <<= 1;
    let mut ws = vec![Wrapping(3u8)];
    ws[0] -= Wrapping(5);
    let x = 5u8;
    println!("{} {} {} {} {} {}", a, b, ws[0], -Wrapping(1u8), !x, !Wrapping(5u8));
}
