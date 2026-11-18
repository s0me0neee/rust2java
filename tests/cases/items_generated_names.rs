// User names that match the words generated temporaries prefer; shadowing; names ending in a digit.
fn main() {
    let value = Some(3);
    let item = vec![Some("a"), None];
    let text = "12 x 7";
    let result: Vec<i32> = text.split(' ').filter_map(|w| w.parse().ok()).collect();
    let tmp = value.map(|v| v * 2);
    let value2 = (value, tmp);
    println!("{:?} {:?} {:?} {:?} {:?}", value, item, result, tmp, value2);
    let x2 = 5;
    let x = 1;
    let x = x + x2;
    let x = x * 2;
    let a1 = 10;
    let a1 = a1 + 1;
    println!("{} {}", x, a1);
    let entry: Vec<(char, i32)> = "ab".chars().zip(1..).collect();
    for (i, c) in entry.iter().enumerate() {
        println!("{} {:?}", i, c);
    }
}
