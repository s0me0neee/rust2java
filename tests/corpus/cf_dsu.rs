struct Dsu {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl Dsu {
    fn new(n: usize) -> Self {
        Dsu { parent: (0..n).collect(), size: vec![1; n] }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) -> bool {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b {
            return false;
        }
        if self.size[a] < self.size[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parent[b] = a;
        self.size[a] += self.size[b];
        true
    }
}

fn main() {
    let n = 8;
    let edges = [(0, 1, 4), (1, 2, 8), (2, 3, 7), (3, 4, 9), (4, 5, 10), (5, 6, 2), (6, 7, 1), (7, 0, 8), (1, 7, 11), (2, 5, 4), (6, 2, 6)];
    let mut sorted = edges.to_vec();
    sorted.sort_by_key(|&(_, _, w)| w);
    let mut dsu = Dsu::new(n);
    let mut total = 0;
    let mut used = Vec::new();
    for (a, b, w) in sorted {
        if dsu.union(a, b) {
            total += w;
            used.push((a, b));
        }
    }
    let roots: std::collections::BTreeSet<usize> = (0..n).map(|i| dsu.find(i)).collect();
    println!("mst {total} edges {:?} components {}", used, roots.len());
}
