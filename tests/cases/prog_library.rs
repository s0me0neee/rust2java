// Library system: enums with struct variants, HashMap<String, Vec<_>>, Result chains, impl blocks, string ops.
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Genre {
    Fiction,
    Science,
    History,
}

#[derive(Debug, Clone)]
struct Book {
    isbn: String,
    title: String,
    year: u16,
    genre: Genre,
    copies: u32,
}

#[derive(Debug, PartialEq)]
enum Event {
    Borrow { member: String, isbn: String },
    Return { member: String, isbn: String, late_days: u32 },
    Join(String),
}

#[derive(Debug)]
enum LibError {
    NoSuchBook(String),
    NoCopies { title: String },
    NotMember(String),
    NotBorrowed,
}

impl fmt::Display for LibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibError::NoSuchBook(i) => write!(f, "no book with isbn {i}"),
            LibError::NoCopies { title } => write!(f, "'{title}' has no copies left"),
            LibError::NotMember(m) => write!(f, "{m} is not a member"),
            LibError::NotBorrowed => f.write_str("book was not borrowed"),
        }
    }
}

#[derive(Default)]
struct Library {
    books: HashMap<String, Book>,
    loans: HashMap<String, Vec<String>>,
    fines: HashMap<String, u32>,
    log: Vec<String>,
}

impl Library {
    fn add(&mut self, isbn: &str, title: &str, year: u16, genre: Genre, copies: u32) {
        self.books.insert(
            isbn.to_string(),
            Book { isbn: isbn.into(), title: title.to_owned(), year, genre, copies },
        );
    }

    fn apply(&mut self, ev: &Event) -> Result<(), LibError> {
        match ev {
            Event::Join(name) => {
                self.loans.entry(name.clone()).or_default();
                self.log.push(format!("+{}", name));
            }
            Event::Borrow { member, isbn } => {
                let loans = self.loans.get_mut(member).ok_or_else(|| LibError::NotMember(member.clone()))?;
                let book = self.books.get_mut(isbn).ok_or(LibError::NoSuchBook(isbn.clone()))?;
                if book.copies == 0 {
                    return Err(LibError::NoCopies { title: book.title.clone() });
                }
                book.copies -= 1;
                loans.push(isbn.clone());
                self.log.push(format!("{} took {}", member, book.title));
            }
            Event::Return { member, isbn, late_days } => {
                let loans = self.loans.get_mut(member).ok_or_else(|| LibError::NotMember(member.clone()))?;
                let idx = loans.iter().position(|i| i == isbn).ok_or(LibError::NotBorrowed)?;
                loans.remove(idx);
                if let Some(b) = self.books.get_mut(isbn) {
                    b.copies += 1;
                }
                if *late_days > 0 {
                    *self.fines.entry(member.clone()).or_insert(0) += late_days * 25;
                }
            }
        }
        Ok(())
    }

    fn by_genre(&self) -> Vec<(Genre, Vec<&str>)> {
        let mut m: HashMap<Genre, Vec<&str>> = HashMap::new();
        for b in self.books.values() {
            m.entry(b.genre).or_insert_with(Vec::new).push(&b.title);
        }
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort();
        for (_, titles) in v.iter_mut() {
            titles.sort_unstable();
        }
        v
    }
}

fn initials(title: &str) -> String {
    title
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .filter_map(|w| w.chars().next())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn main() {
    let mut lib = Library::default();
    lib.add("111", "the rust programming language", 2018, Genre::Science, 2);
    lib.add("222", "a brief history of time", 1988, Genre::Science, 1);
    lib.add("333", "war and peace", 1869, Genre::Fiction, 1);
    lib.add("444", "the guns of august", 1962, Genre::History, 0);

    let events = vec![
        Event::Join("ann".into()),
        Event::Join("bob".into()),
        Event::Borrow { member: "ann".into(), isbn: "111".into() },
        Event::Borrow { member: "bob".into(), isbn: "333".into() },
        Event::Borrow { member: "ann".into(), isbn: "333".into() },
        Event::Borrow { member: "cid".into(), isbn: "111".into() },
        Event::Borrow { member: "bob".into(), isbn: "999".into() },
        Event::Borrow { member: "bob".into(), isbn: "444".into() },
        Event::Return { member: "bob".into(), isbn: "333".into(), late_days: 3 },
        Event::Return { member: "ann".into(), isbn: "222".into(), late_days: 0 },
        Event::Borrow { member: "ann".into(), isbn: "333".into() },
    ];
    let mut errors = 0;
    for (n, ev) in events.iter().enumerate() {
        if let Err(e) = lib.apply(ev) {
            errors += 1;
            println!("event {n:>2} failed: {e}");
        }
    }
    println!("{} errors; log: {}", errors, lib.log.join("; "));

    let mut members: Vec<_> = lib.loans.iter().collect();
    members.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    for (m, l) in &members {
        println!("{m}: {l:?} fine={}", lib.fines.get(*m).copied().unwrap_or_default());
    }
    for (g, titles) in lib.by_genre() {
        println!("{:<8}|{}", format!("{:?}", g), titles.join(" / "));
    }
    let oldest = lib.books.values().min_by_key(|b| b.year).unwrap();
    println!("oldest: {} ({}) {}", oldest.title, oldest.year, initials(&oldest.title));
    let mut isbns: Vec<&String> = lib.books.keys().collect();
    isbns.sort();
    let available: Vec<_> = isbns
        .iter()
        .map(|i| &lib.books[*i])
        .filter(|b| b.copies > 0)
        .map(|b| format!("{}x{}", b.isbn, b.copies))
        .collect();
    println!("{}", available.join(","));
    println!("{:?}", events.last());
    println!("{}", events.contains(&Event::Join("bob".to_string())));
    let title_caps: Vec<String> = lib.books["111"]
        .title
        .split(' ')
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect();
    println!("{}", title_caps.join(" "));
}
