// `&mut node.left` / `&mut c.hits` / `&mut v[i]` passed to a function: a cell, the call, a write-back.
struct Node {
    val: i32,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

fn insert(node: &mut Option<Box<Node>>, v: i32) {
    match node {
        None => *node = Some(Box::new(Node { val: v, left: None, right: None })),
        Some(n) => {
            if v < n.val {
                insert(&mut n.left, v)
            } else {
                insert(&mut n.right, v);
            }
        }
    }
}

fn inorder(node: &Option<Box<Node>>, out: &mut Vec<i32>) {
    if let Some(n) = node {
        inorder(&n.left, out);
        out.push(n.val);
        inorder(&n.right, out);
    }
}

struct Counter { hits: u32, label: String }
fn bump(n: &mut u32) { *n += 1; }
fn shout(s: &mut String) { s.push('!'); }

fn main() {
    let mut root = None;
    for v in [5, 2, 8, 1, 9, 3] {
        insert(&mut root, v);
    }
    let mut out = Vec::new();
    inorder(&root, &mut out);
    println!("{:?}", out);
    let mut c = Counter { hits: 0, label: "hi".to_string() };
    bump(&mut c.hits);
    bump(&mut c.hits);
    shout(&mut c.label);
    let mut counts = vec![0u32; 3];
    bump(&mut counts[1]);
    println!("{} {} {:?}", c.hits, c.label, counts);
}
