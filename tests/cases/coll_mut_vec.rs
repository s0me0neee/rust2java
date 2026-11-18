fn main() {
    let mut values = vec![1, 2, 3];
    if let Some(x) = values.get_mut(0) {
        *x += 10;
    }
    if let Some(x) = values.last_mut() {
        *x = 30;
    }
    if let Some(x) = values.first_mut() {
        *x *= 2;
    }
    if let Some(x) = values.get_mut(9) {
        *x = 0;
    }
    for x in values.iter_mut() {
        *x += 1;
    }
    println!("{:?}", values);
    let mut rows = vec![vec![1, 2], vec![3, 4]];
    for row in rows.iter_mut() {
        row[0] *= 10;
    }
    println!("{:?}", rows);
    let mut names = vec![String::from("a")];
    if let Some(name) = names.first_mut() {
        name.push_str("b");
    }
    println!("{:?}", names);
}
