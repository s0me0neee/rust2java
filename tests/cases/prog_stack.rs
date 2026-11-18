// Generics: a Stack<T>, an ordered container bounded by Ord, a generic function
// with a closure parameter and a tiny generic table with a Display bound.

trait Summary {
    fn summary(&self) -> String;
}

use std::fmt;

struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    fn new() -> Stack<T> {
        Stack { items: Vec::new() }
    }

    fn push(&mut self, item: T) {
        self.items.push(item);
    }

    fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    fn peek(&self) -> Option<&T> {
        self.items.last()
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn clear(&mut self) {
        self.items.clear();
    }
}

impl<T: fmt::Display> Stack<T> {
    fn render(&self) -> String {
        let parts: Vec<String> = self.items.iter().map(|item| item.to_string()).collect();
        format!("[{}]", parts.join(", "))
    }
}

impl<T: fmt::Display> Summary for Stack<T> {
    fn summary(&self) -> String {
        format!("stack of {} ({})", self.len(), self.render())
    }
}

impl<T> Stack<T> {
    fn as_slice(&self) -> &[T] {
        &self.items
    }
}

fn stack_sum(stack: &Stack<i32>) -> i32 {
    stack.as_slice().iter().sum()
}

fn stack_max(stack: &Stack<i32>) -> Option<i32> {
    stack.as_slice().iter().copied().max()
}

struct SortedVec<T: Ord + Clone> {
    items: Vec<T>,
}

impl<T: Ord + Clone> SortedVec<T> {
    fn new() -> SortedVec<T> {
        SortedVec { items: Vec::new() }
    }

    fn insert(&mut self, item: T) {
        let position = self.items.iter().position(|existing| existing > &item);
        match position {
            Some(index) => self.items.insert(index, item),
            None => self.items.push(item),
        }
    }

    fn contains(&self, item: &T) -> bool {
        self.items.binary_search(item).is_ok()
    }

    fn smallest(&self) -> Option<&T> {
        self.items.first()
    }

    fn largest(&self) -> Option<&T> {
        self.items.last()
    }

    fn extend_all(&mut self, more: Vec<T>) {
        for item in more {
            self.insert(item);
        }
    }
}

fn largest<T: Ord + Clone>(items: &[T]) -> Option<T> {
    let mut best: Option<T> = None;
    for item in items {
        best = match best {
            Some(current) if current >= *item => Some(current),
            _ => Some(item.clone()),
        };
    }
    best
}

fn count_if<T, F: Fn(&T) -> bool>(items: &[T], predicate: F) -> usize {
    items.iter().filter(|item| predicate(item)).count()
}

fn map_all<T, U, F: Fn(T) -> U>(items: Vec<T>, f: F) -> Vec<U> {
    items.into_iter().map(f).collect()
}

fn zip_pairs<A: fmt::Display, B: fmt::Display>(left: &[A], right: &[B]) -> Vec<String> {
    left.iter()
        .zip(right.iter())
        .map(|(a, b)| format!("{}={}", a, b))
        .collect()
}

struct Table<T> {
    title: String,
    rows: Vec<T>,
}

impl<T: fmt::Display> fmt::Display for Table<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "+--{}--+", "-".repeat(self.title.len()))?;
        writeln!(f, "| {} |", self.title)?;
        for row in &self.rows {
            writeln!(f, "| {} |", row)?;
        }
        write!(f, "+--{}--+", "-".repeat(self.title.len()))
    }
}

fn main() {
    let mut numbers: Stack<i32> = Stack::new();
    for value in [3, 1, 4, 1, 5, 9, 2, 6] {
        numbers.push(value);
    }
    println!("{}", numbers.render());
    println!("len {} empty {}", numbers.len(), numbers.is_empty());
    println!("peek {:?}", numbers.peek());
    println!("pop {:?}", numbers.pop());
    println!("after pop: {}", numbers.summary());
    println!("sum {} max {:?}", stack_sum(&numbers), stack_max(&numbers));
    let mut words: Stack<String> = Stack::new();
    for word in ["alpha", "beta", "gamma"] {
        words.push(word.to_string());
    }
    println!("{}", words.render());
    println!("{}", words.summary());
    println!("popped word: {:?}", words.pop());
    let mut empty: Stack<i32> = Stack::new();
    println!("empty summary: {}", empty.summary());
    empty.push(42);
    let drained = empty.pop();
    println!("drained {:?} then {:?}", drained, empty.pop());
    let mut sorted: SortedVec<i32> = SortedVec::new();
    sorted.extend_all(vec![5, 3, 8, 1, 9, 2]);
    println!(
        "sorted smallest {:?} largest {:?}",
        sorted.smallest(),
        sorted.largest()
    );
    println!("contains 8: {}, contains 7: {}", sorted.contains(&8), sorted.contains(&7));
    let mut words_sorted: SortedVec<String> = SortedVec::new();
    words_sorted.insert("pear".to_string());
    words_sorted.insert("apple".to_string());
    words_sorted.insert("fig".to_string());
    println!("words: {:?}", words_sorted.smallest());
    let top = largest(&[4, 9, 2, 9, 1]);
    println!("largest: {:?}", top);
    println!("count even: {}", count_if(&[1, 2, 3, 4, 5, 6], |n| n % 2 == 0));
    let doubled = map_all(vec![1, 2, 3], |n| n * 2);
    println!("doubled: {:?}", doubled);
    let labels = map_all(vec![1, 2], |n| format!("#{}", n));
    println!("labels: {:?}", labels);
    println!("zip: {:?}", zip_pairs(&["a", "b"], &[1, 2]));
    let table = Table {
        title: "scores".to_string(),
        rows: vec!["alice 10", "bob 7"],
    };
    println!("{}", table);
    let generic_table = Table {
        title: "nums".to_string(),
        rows: vec![1, 22, 333],
    };
    println!("{}", generic_table);
    let mut stack: Stack<f64> = Stack::new();
    stack.push(1.5);
    stack.push(2.25);
    println!("{}", stack.render());
}
