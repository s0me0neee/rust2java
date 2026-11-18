// Recursive data structures: a binary search tree and a singly linked list, both
// built on Option<Box<Node>>, with traversal, height, search and reversal.
#[derive(Debug, Clone, PartialEq)]
struct TreeNode {
    value: i32,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

impl TreeNode {
    fn leaf(value: i32) -> TreeNode {
        TreeNode {
            value,
            left: None,
            right: None,
        }
    }

    fn insert(&mut self, value: i32) -> bool {
        if value == self.value {
            return false;
        }
        if value < self.value {
            match self.left.as_mut() {
                Some(child) => child.insert(value),
                None => {
                    self.left = Some(Box::new(TreeNode::leaf(value)));
                    true
                }
            }
        } else {
            match self.right.as_mut() {
                Some(child) => child.insert(value),
                None => {
                    self.right = Some(Box::new(TreeNode::leaf(value)));
                    true
                }
            }
        }
    }

    fn contains(&self, value: i32) -> bool {
        if value == self.value {
            true
        } else if value < self.value {
            self.left.as_ref().is_some_and(|child| child.contains(value))
        } else {
            self.right.as_ref().is_some_and(|child| child.contains(value))
        }
    }

    fn inorder(&self, out: &mut Vec<i32>) {
        if let Some(left) = &self.left {
            left.inorder(out);
        }
        out.push(self.value);
        if let Some(right) = &self.right {
            right.inorder(out);
        }
    }

    fn height(&self) -> usize {
        let left = self.left.as_ref().map_or(0, |child| child.height());
        let right = self.right.as_ref().map_or(0, |child| child.height());
        1 + left.max(right)
    }

    fn count(&self) -> usize {
        1 + self.left.as_ref().map_or(0, |c| c.count()) + self.right.as_ref().map_or(0, |c| c.count())
    }

    fn min(&self) -> i32 {
        match &self.left {
            Some(child) => child.min(),
            None => self.value,
        }
    }

    fn max(&self) -> i32 {
        match &self.right {
            Some(child) => child.max(),
            None => self.value,
        }
    }

    fn sum(&self) -> i32 {
        self.value
            + self.left.as_ref().map_or(0, |c| c.sum())
            + self.right.as_ref().map_or(0, |c| c.sum())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ListNode {
    value: i32,
    next: Option<Box<ListNode>>,
}

fn push_front(head: Option<Box<ListNode>>, value: i32) -> Option<Box<ListNode>> {
    Some(Box::new(ListNode { value, next: head }))
}

fn list_from(values: &[i32]) -> Option<Box<ListNode>> {
    let mut head = None;
    for value in values.iter().rev() {
        head = push_front(head, *value);
    }
    head
}

fn reverse(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut prev = None;
    while let Some(mut node) = head {
        head = node.next.take();
        node.next = prev;
        prev = Some(node);
    }
    prev
}

fn list_to_vec(mut head: Option<&ListNode>) -> Vec<i32> {
    let mut out = Vec::new();
    while let Some(node) = head {
        out.push(node.value);
        head = node.next.as_deref();
    }
    out
}

fn merge(a: Option<Box<ListNode>>, b: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    match (a, b) {
        (None, other) => other,
        (other, None) => other,
        (Some(x), Some(y)) => {
            if x.value <= y.value {
                Some(Box::new(ListNode {
                    value: x.value,
                    next: merge(x.next, Some(y)),
                }))
            } else {
                Some(Box::new(ListNode {
                    value: y.value,
                    next: merge(Some(x), y.next),
                }))
            }
        }
    }
}

fn main() {
    let values = [50, 30, 70, 20, 40, 60, 80, 35, 45, 65];
    let mut tree: Option<Box<TreeNode>> = None;
    for value in values {
        match tree.as_mut() {
            Some(root) => {
                if root.insert(value) {
                    println!("inserted {}", value);
                } else {
                    println!("duplicate {}", value);
                }
            }
            None => {
                tree = Some(Box::new(TreeNode::leaf(value)));
                println!("root {}", value);
            }
        }
    }
    if let Some(root) = &tree {
        let mut ordered = Vec::new();
        root.inorder(&mut ordered);
        println!("inorder: {:?}", ordered);
        println!(
            "count {} height {} sum {} min {} max {}",
            root.count(),
            root.height(),
            root.sum(),
            root.min(),
            root.max()
        );
        for probe in [40, 41, 80, 5] {
            println!("contains {}: {}", probe, root.contains(probe));
        }
        let evens: Vec<i32> = ordered.iter().copied().filter(|v| v % 2 == 0).collect();
        println!("evens: {:?}", evens);
        println!("average: {:.2}", root.sum() as f64 / root.count() as f64);
    }
    let head = list_from(&[1, 2, 3, 4, 5]);
    println!("list: {:?}", list_to_vec(head.as_deref()));
    let reversed = reverse(head);
    println!("reversed: {:?}", list_to_vec(reversed.as_deref()));
    println!("{:?}", list_to_vec(reversed.as_deref()).first());
    let left = list_from(&[1, 4, 7]);
    let right = list_from(&[2, 3, 9, 12]);
    let merged = merge(left, right);
    println!("merged: {:?}", list_to_vec(merged.as_deref()));
    let empty: Option<Box<ListNode>> = None;
    println!("reversed empty: {:?}", list_to_vec(reverse(empty).as_deref()));
    println!("push: {:?}", list_to_vec(push_front(None, 99).as_deref()));
    let cloned = tree.clone();
    println!("tree unchanged after clone: {}", cloned.is_some());
    println!("tree is empty: {}", tree.is_none());
}
