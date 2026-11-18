// read_line into a reused buffer, `if let Some(&x) = v.get(i)`, immutable arrays/vecs, map.get.
fn main() {
    let mut buf = String::new();
    let hash = ['a', 'e', 'i', 'o', 'u'];
    let stdin = std::io::stdin();
    stdin.read_line(&mut buf).unwrap();
    let t = buf.trim().parse::<u32>().unwrap();
    for _ in 0..t {
        buf.clear();
        stdin.read_line(&mut buf).unwrap();
        let line: Vec<char> = buf.trim().chars().collect();
        let mut res = String::new();
        let mut skip = false;
        for (i, c) in line.iter().enumerate() {
            if skip {
                skip = false;
                continue;
            }
            if hash.contains(&line[i]) {
                if let Some(&n) = line.get(i + 1) {
                    res.push(n);
                }
                skip = true;
            }
            let _ = c;
        }
        println!("{res}");
    }
    let shared = vec![3, 1, 2];
    let mut copy = shared.clone();
    copy.sort();
    let moved = shared;
    let mut owner = moved;
    owner.push(9);
    println!("{:?} {:?}", copy, owner);
    let mut map = std::collections::HashMap::new();
    map.insert("k", 1);
    if let Some(v) = map.get("k") {
        println!("{}", v + 1);
    }
    if let Some(v) = map.get("x") {
        println!("{}", v);
    }
}
