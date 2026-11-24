#[derive(Debug)]
struct Node<T> {
    val: T,
    next: Option<Box<Node<T>>>,
}

#[derive(Debug)]
struct List<T> {
    head: Option<Box<Node<T>>>,
    len: usize,
}

impl<T: std::fmt::Debug + PartialOrd + Copy> List<T> {
    fn new() -> Self {
        List { head: None, len: 0 }
    }

    fn push_front(&mut self, val: T) {
        let old = self.head.take();
        self.head = Some(Box::new(Node { val, next: old }));
        self.len += 1;
    }

    fn pop_front(&mut self) -> Option<T> {
        self.head.take().map(|n| {
            self.head = n.next;
            self.len -= 1;
            n.val
        })
    }

    fn reverse(&mut self) {
        let mut prev: Option<Box<Node<T>>> = None;
        let mut cur = self.head.take();
        while let Some(mut node) = cur {
            cur = node.next.take();
            node.next = prev;
            prev = Some(node);
        }
        self.head = prev;
    }

    fn to_vec(&self) -> Vec<T> {
        let mut out = Vec::new();
        let mut cur = &self.head;
        while let Some(node) = cur {
            out.push(node.val);
            cur = &node.next;
        }
        out
    }

    fn max(&self) -> Option<T> {
        let mut cur = self.head.as_ref();
        let mut best: Option<T> = None;
        while let Some(n) = cur {
            if best.map_or(true, |b| n.val > b) {
                best = Some(n.val);
            }
            cur = n.next.as_ref();
        }
        best
    }
}

fn main() {
    let mut l = List::new();
    for x in [3, 1, 4, 1, 5, 9, 2, 6] {
        l.push_front(x);
    }
    println!("{:?} len={}", l.to_vec(), l.len);
    l.reverse();
    println!("{:?} max={:?}", l.to_vec(), l.max());
    println!("{:?} {:?}", l.pop_front(), l.pop_front());
    println!("{:?} len={}", l.to_vec(), l.len);
    let empty: List<i32> = List::new();
    println!("{:?} {:?}", empty.max(), empty.head.is_none());
}
