use std::collections::BTreeMap;

fn main() {
    let names = vec!["ann", "bob", "cy"];
    for (i, name) in names.iter().enumerate() {
        println!("{} {}", i, name);
    }
    for i in (0..10).step_by(4) {
        print!("{i},");
    }
    println!();
    for i in (1..=3).rev() {
        print!("{i} ");
    }
    println!();
    let mut nums = vec![1, 2, 3];
    for n in nums.iter_mut() {
        *n *= 10;
    }
    for n in &mut nums {
        *n += 1;
    }
    println!("{:?}", nums);
    let mut ages = BTreeMap::new();
    ages.insert("zed", 30);
    ages.insert("amy", 25);
    for (name, age) in &ages {
        println!("{name} is {age}");
    }
    for c in "héllo".chars() {
        print!("[{c}]");
    }
    println!();
    'outer: for a in 0..5 {
        for b in 0..5 {
            if a * b == 6 {
                println!("found {a} {b}");
                break 'outer;
            }
        }
    }
    let words = "the quick  brown fox";
    for w in words.split_whitespace() {
        print!("<{w}>");
    }
    println!();
    for part in "a,b,,c".split(',') {
        print!("({part})");
    }
    println!();
    for line in "one\ntwo\nthree".lines() {
        println!("{}", line.len());
    }
}
