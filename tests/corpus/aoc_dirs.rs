use std::collections::BTreeMap;

fn main() {
    let log = "$ cd /\n$ ls\ndir a\n14848514 b.txt\n8504156 c.dat\ndir d\n$ cd a\n$ ls\ndir e\n29116 f\n2557 g\n62596 h.lst\n$ cd e\n$ ls\n584 i\n$ cd ..\n$ cd ..\n$ cd d\n$ ls\n4060174 j\n8033020 d.log\n5626152 d.ext\n7214296 k";
    let mut cwd: Vec<String> = Vec::new();
    let mut sizes: BTreeMap<String, u64> = BTreeMap::new();
    for line in log.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["$", "cd", "/"] => cwd.clear(),
            ["$", "cd", ".."] => {
                cwd.pop();
            }
            ["$", "cd", name] => cwd.push(name.to_string()),
            ["$", "ls"] | ["dir", _] => {}
            [size, _] => {
                let size: u64 = size.parse().unwrap();
                for i in 0..=cwd.len() {
                    let path = format!("/{}", cwd[..i].join("/"));
                    *sizes.entry(path).or_insert(0) += size;
                }
            }
            _ => unreachable!(),
        }
    }
    let small: u64 = sizes.values().filter(|&&s| s <= 100_000).sum();
    let need = 30_000_000 - (70_000_000 - sizes["/"]);
    let del = sizes.values().copied().filter(|&s| s >= need).min().unwrap();
    for (path, size) in &sizes {
        println!("{path:<8} {size:>10}");
    }
    println!("{small} {del}");
}
