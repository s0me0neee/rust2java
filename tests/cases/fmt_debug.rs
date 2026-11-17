use std::collections::HashMap;

fn main() {
    println!("{:?} {:?}", "a\"\\\n\t", '\'');
    println!("{:?}", "\u{7}\u{1b}\u{85}");
    println!("{:?} {:?}", (), vec![(), ()]);
    println!("{:?} {:?} {:?} {:?}", 1.0, 1e16, 0.0001, 0.00001);
    println!("{:?} {:?}", -0.0, 12345678.5);
    println!("{:?} {:?}", 0.0 / 0.0, 1.0 / 0.0);
    let values = vec![Some("one"), None, Some("two")];
    println!("{:?}", values);
    let pair = ("hi", Some(3));
    println!("{:?}", pair);
    let result: Result<&str, i32> = Ok("ok");
    println!("{:?}", result);
    let count: Result<i32, &str> = Ok(3);
    let mut width = 5;
    println!("{:0width$?} {:0width$?}", count, vec![1, 2]);
    width = 3;
    println!("{}", width);
    let map: HashMap<i32, &str> = HashMap::new();
    println!("{:?}", map);
}
