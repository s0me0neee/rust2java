// Shared mutable graph: Rc<RefCell<Room>> nodes with Weak back-links, borrow/borrow_mut,
// Rc::clone/strong_count, a scripted walk, and an observer list of boxed closures.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

#[derive(Debug)]
struct Room {
    name: String,
    items: Vec<String>,
    exits: HashMap<char, Rc<RefCell<Room>>>,
    parent: Option<Weak<RefCell<Room>>>,
    visits: u32,
}

fn room(name: &str, items: &[&str]) -> Rc<RefCell<Room>> {
    Rc::new(RefCell::new(Room {
        name: name.to_string(),
        items: items.iter().map(|s| s.to_string()).collect(),
        exits: HashMap::new(),
        parent: None,
        visits: 0,
    }))
}

fn link(a: &Rc<RefCell<Room>>, dir: char, b: &Rc<RefCell<Room>>) {
    a.borrow_mut().exits.insert(dir, Rc::clone(b));
    b.borrow_mut().parent = Some(Rc::downgrade(a));
}

struct Player {
    at: Rc<RefCell<Room>>,
    bag: Vec<String>,
    log: Vec<Box<dyn Fn(&str) -> Option<String>>>,
}

impl Player {
    fn go(&mut self, dir: char) -> bool {
        let next = self.at.borrow().exits.get(&dir).cloned();
        match next {
            Some(r) => {
                r.borrow_mut().visits += 1;
                self.at = r;
                true
            }
            None => false,
        }
    }
    fn back(&mut self) -> bool {
        let up = self.at.borrow().parent.as_ref().and_then(|w| w.upgrade());
        if let Some(p) = up {
            self.at = p;
            true
        } else {
            false
        }
    }
    fn take_all(&mut self) -> usize {
        let mut here = self.at.borrow_mut();
        let n = here.items.len();
        self.bag.extend(here.items.drain(..));
        n
    }
    fn notify(&self, event: &str) {
        for f in &self.log {
            if let Some(msg) = f(event) {
                println!("  [{}]", msg);
            }
        }
    }
}

fn main() {
    let hall = room("hall", &["lamp"]);
    let lib = room("library", &["book", "scroll"]);
    let vault = room("vault", &["gold", "gem", "key"]);
    link(&hall, 'n', &lib);
    link(&lib, 'e', &vault);
    let mut p = Player { at: Rc::clone(&hall), bag: vec![], log: vec![] };
    p.log.push(Box::new(|e| if e.starts_with("took") { Some(e.to_uppercase()) } else { None }));
    let prefix = String::from("moved");
    p.log.push(Box::new(move |e| e.strip_prefix("go ").map(|d| format!("{} {}", prefix, d))));
    for step in "n e x b b b n".split(' ') {
        let c = step.chars().next().unwrap();
        let ok = match c {
            'b' => p.back(),
            d => p.go(d),
        };
        let name = p.at.borrow().name.clone();
        println!("{} -> {} ({})", step, name, if ok { "ok" } else { "blocked" });
        p.notify(&format!("go {}", c));
        if ok && c != 'b' {
            let n = p.take_all();
            p.notify(&format!("took {} in {}", n, name));
        }
    }
    p.bag.sort();
    println!("bag {:?}", p.bag);
    for r in [&hall, &lib, &vault] {
        let r = r.borrow();
        println!("{}: visits={} items={:?} exits={:?}", r.name, r.visits, r.items, {
            let mut k: Vec<_> = r.exits.keys().collect();
            k.sort();
            k
        });
    }
    let up: Option<Rc<RefCell<Room>>> = vault.borrow().parent.as_ref().and_then(|w| w.upgrade());
    let orphan_parent = up.map(|r| r.borrow().name.clone());
    println!("{:?}", orphan_parent);
}
