#[derive(Debug, Clone)]
struct Team {
    name: String,
    solved: u32,
    penalty: u32,
}

fn main() {
    let raw = "alpha 5 300\nbeta 5 250\ngamma 3 100\ndelta 5 250\nepsilon 0 0";
    let mut teams: Vec<Team> = raw
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            Team { name: f[0].to_owned(), solved: f[1].parse().unwrap(), penalty: f[2].parse().unwrap() }
        })
        .collect();
    teams.sort_by(|a, b| b.solved.cmp(&a.solved).then(a.penalty.cmp(&b.penalty)).then_with(|| a.name.cmp(&b.name)));
    let mut rank = 0;
    let mut prev: Option<(u32, u32)> = None;
    for (i, t) in teams.iter().enumerate() {
        if prev != Some((t.solved, t.penalty)) {
            rank = i + 1;
        }
        prev = Some((t.solved, t.penalty));
        println!("{rank:>2}. {:<8} {} {}", t.name, t.solved, t.penalty);
    }
    let mut names: Vec<String> = teams.iter().map(|t| t.name.clone()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    names.dedup_by_key(|n| n.len());
    println!("{:?}", names);
}
