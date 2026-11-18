// `&mut Option<T>` locals and parameters: writes through the reference reach the original.
struct Node {
    value: i32,
    next: Option<Box<Node>>,
}

fn set(o: &mut Option<Box<Node>>, v: i32) {
    *o = Some(Box::new(Node { value: v, next: None }));
}

fn push_front(head: &mut Option<Box<Node>>, v: i32) {
    let old = head.take();
    *head = Some(Box::new(Node { value: v, next: old }));
}

fn bump(o: &mut Option<i32>) {
    *o = Some(o.map_or(0, |x| x + 1));
}

fn count(head: &Option<Box<Node>>) -> usize {
    let mut n = 0;
    let mut cur = head;
    while let Some(node) = cur {
        n += 1;
        cur = &node.next;
    }
    n
}

fn main() {
    let mut head: Option<Box<Node>> = None;
    set(&mut head, 9);
    push_front(&mut head, 8);
    push_front(&mut head, 7);
    println!("{} {}", count(&head), head.as_ref().map(|n| n.value).unwrap_or(-1));

    let mut c: Option<i32> = None;
    bump(&mut c);
    bump(&mut c);
    println!("{:?}", c);
    let r = &mut c;
    let old = r.replace(40);
    *r = r.map(|x| x + 2);
    println!("{:?} {:?}", old, c);

    let mut best: Option<i32> = None;
    let mut consider = |x: i32| {
        if best.map_or(true, |b| x > b) {
            best = Some(x);
        }
    };
    consider(3);
    consider(9);
    consider(4);
    println!("{:?}", best);
}
