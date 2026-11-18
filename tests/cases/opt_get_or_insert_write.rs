// get_or_insert(_with) followed by a write on String/number/Vec contents; constructor paths as values.
fn main() {
    let mut slot: Option<String> = None;
    slot.get_or_insert_with(|| "x".to_string()).push('y');
    slot.get_or_insert_with(String::new).push_str("z");
    let mut n: Option<i32> = None;
    *n.get_or_insert(5) += 1;
    let mut v: Option<Vec<i32>> = None;
    v.get_or_insert_with(Vec::new).push(3);
    println!("{:?} {:?} {:?}", slot, n, v);
}
