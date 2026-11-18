use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Reverse;

fn main() {
    let mut queue = VecDeque::new();
    queue.push_back(1);
    queue.push_back(2);
    queue.push_front(0);
    println!("{:?} {:?}", queue.get(1), queue.get(8));
    println!("{} {:?}", queue[1], queue.iter().rev().copied().collect::<Vec<_>>());
    queue.make_contiguous().reverse();
    println!("{:?}", queue);
    queue.make_contiguous().sort();
    println!("{:?}", queue);
    let mut heap = BinaryHeap::new();
    heap.push(3);
    heap.push(1);
    heap.push(2);
    println!("{:?}", heap.into_sorted_vec());
    let mut min_heap = BinaryHeap::new();
    min_heap.push(Reverse(5));
    min_heap.push(Reverse(2));
    if let Some(Reverse(value)) = min_heap.pop() {
        println!("{}", value);
    }
    let mut pairs = BinaryHeap::new();
    pairs.push((2, 1));
    pairs.push((9, 0));
    println!("{:?}", pairs.pop());
    let mut heap = BinaryHeap::new();
    heap.push(3);
    heap.push(1);
    heap.push(2);
    let mut contents = heap.into_vec();
    contents.sort();
    println!("{:?}", contents);
}
