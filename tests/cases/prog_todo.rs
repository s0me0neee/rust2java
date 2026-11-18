// Todo list: a tiny command language parsed from stdin into an enum, applied to
// an in-memory list of tasks, and reported through Display impls.
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum Command {
    Add(String),
    Done(usize),
    Remove(usize),
    Rename(usize, String),
    List,
    Stats,
    Clear,
    Quit,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Add(title) => write!(f, "add {:?}", title),
            Command::Done(id) => write!(f, "done {}", id),
            Command::Remove(id) => write!(f, "remove {}", id),
            Command::Rename(id, title) => write!(f, "rename {} {:?}", id, title),
            Command::List => write!(f, "list"),
            Command::Stats => write!(f, "stats"),
            Command::Clear => write!(f, "clear"),
            Command::Quit => write!(f, "quit"),
        }
    }
}

#[derive(Debug, Clone)]
struct Task {
    id: usize,
    title: String,
    done: bool,
}

impl Task {
    fn new(id: usize, title: String) -> Task {
        Task {
            id,
            title,
            done: false,
        }
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mark = if self.done { 'x' } else { ' ' };
        write!(f, "[{}] {:>2}. {}", mark, self.id, self.title)
    }
}

fn parse_command(line: &str) -> Result<Command, String> {
    let mut parts = line.split_whitespace();
    let name = parts.next().ok_or_else(|| "empty command".to_string())?;
    let rest: Vec<&str> = parts.collect();
    match name {
        "add" if !rest.is_empty() => Ok(Command::Add(rest.join(" "))),
        "done" if rest.len() == 1 => {
            let id = rest[0].parse::<usize>().map_err(|e| e.to_string())?;
            Ok(Command::Done(id))
        }
        "remove" if rest.len() == 1 => match rest[0].parse::<usize>() {
            Ok(id) => Ok(Command::Remove(id)),
            Err(e) => Err(format!("bad id: {}", e)),
        },
        "rename" if rest.len() >= 2 => {
            let id = rest[0].parse::<usize>().map_err(|e| e.to_string())?;
            Ok(Command::Rename(id, rest[1..].join(" ")))
        }
        "list" => Ok(Command::List),
        "stats" => Ok(Command::Stats),
        "clear" => Ok(Command::Clear),
        "quit" => Ok(Command::Quit),
        _ => Err(format!("unknown command: {}", name)),
    }
}

struct Todo {
    tasks: Vec<Task>,
    next_id: usize,
}

impl Todo {
    fn new() -> Todo {
        Todo {
            tasks: Vec::new(),
            next_id: 1,
        }
    }

    fn add(&mut self, title: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task::new(id, title));
        id
    }

    fn find(&mut self, id: usize) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|task| task.id == id)
    }

    fn apply(&mut self, command: Command) -> bool {
        match command {
            Command::Add(title) => {
                let id = self.add(title);
                println!("added #{}", id);
            }
            Command::Done(id) => match self.find(id) {
                Some(task) => {
                    task.done = true;
                    println!("completed {}", task);
                }
                None => println!("no task #{}", id),
            },
            Command::Remove(id) => {
                let before = self.tasks.len();
                self.tasks.retain(|task| task.id != id);
                if self.tasks.len() == before {
                    println!("no task #{}", id);
                } else {
                    println!("removed #{}", id);
                }
            }
            Command::Rename(id, title) => match self.find(id) {
                Some(task) => {
                    let old = std::mem::replace(&mut task.title, title);
                    println!("renamed {:?} to {:?}", old, task.title);
                }
                None => println!("no task #{}", id),
            },
            Command::List => self.list(),
            Command::Stats => self.stats(),
            Command::Clear => {
                self.tasks.clear();
                println!("cleared");
            }
            Command::Quit => return false,
        }
        true
    }

    fn list(&self) {
        if self.tasks.is_empty() {
            println!("(no tasks)");
            return;
        }
        for task in &self.tasks {
            println!("{}", task);
        }
    }

    fn stats(&self) {
        let done = self.tasks.iter().filter(|task| task.done).count();
        let open = self.tasks.len() - done;
        let longest = self
            .tasks
            .iter()
            .map(|task| task.title.len())
            .max()
            .unwrap_or(0);
        println!(
            "{} open, {} done, longest title {} chars",
            open, done, longest
        );
        let mut words = 0;
        for task in &self.tasks {
            words += task.title.split_whitespace().count();
        }
        println!("total title words: {}", words);
    }
}

fn main() {
    let mut todo = Todo::new();
    let script = vec![
        "add buy milk",
        "add write the report",
        "add call alice",
        "list",
        "done 2",
        "rename 3 call bob",
        "list",
        "stats",
        "remove 9",
        "done 9",
        "bogus 1",
        "clear",
        "list",
        "quit",
        "add ignored",
    ];
    for line in script {
        let command = match parse_command(line) {
            Ok(command) => command,
            Err(message) => {
                println!("error: {}", message);
                continue;
            }
        };
        println!("> {}", command);
        if !todo.apply(command) {
            println!("bye");
            break;
        }
    }
}
