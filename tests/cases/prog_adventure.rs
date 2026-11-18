// A tiny text adventure: rooms in an enum, commands parsed into another enum,
// a state machine with items, locked doors and a score at the end.
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Room {
    Hall,
    Library,
    Cellar,
    Vault,
    Garden,
}

impl Room {
    fn title(&self) -> &'static str {
        match self {
            Room::Hall => "Grand Hall",
            Room::Library => "Dusty Library",
            Room::Cellar => "Cold Cellar",
            Room::Vault => "Sealed Vault",
            Room::Garden => "Overgrown Garden",
        }
    }

    fn describe(&self) -> &'static str {
        match self {
            Room::Hall => "A chandelier hangs above a cracked marble floor.",
            Room::Library => "Shelves sag under the weight of forgotten books.",
            Room::Cellar => "Barrels line the walls; something skitters in the dark.",
            Room::Vault => "Gold coins gleam in the lamplight.",
            Room::Garden => "Roses climb a rusted iron gate.",
        }
    }
}

impl fmt::Display for Room {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.title(), self)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Command {
    Go(String),
    Take(String),
    Use(String),
    Look,
    Inventory,
    Quit,
}

fn parse(input: &str) -> Result<Command, String> {
    let words: Vec<&str> = input.split_whitespace().collect();
    if words.is_empty() {
        return Err("say something".to_string());
    }
    match words[0] {
        "go" if words.len() == 2 => Ok(Command::Go(words[1].to_string())),
        "take" if words.len() >= 2 => Ok(Command::Take(words[1..].join(" "))),
        "use" if words.len() >= 2 => Ok(Command::Use(words[1..].join(" "))),
        "look" => Ok(Command::Look),
        "inventory" | "i" => Ok(Command::Inventory),
        "quit" => Ok(Command::Quit),
        other => Err(format!("I don't know how to `{}`", other)),
    }
}

struct Game {
    room: Room,
    inventory: Vec<String>,
    flags: HashSet<String>,
    moves: usize,
    finished: bool,
}

impl Game {
    fn new() -> Game {
        Game {
            room: Room::Hall,
            inventory: Vec::new(),
            flags: HashSet::new(),
            moves: 0,
            finished: false,
        }
    }

    fn has(&self, item: &str) -> bool {
        self.inventory.iter().any(|held| held == item)
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }

    fn go(&mut self, direction: &str) -> String {
        let next = match (self.room, direction) {
            (Room::Hall, "north") => Some(Room::Library),
            (Room::Hall, "south") => Some(Room::Garden),
            (Room::Library, "south") => Some(Room::Hall),
            (Room::Library, "down") => {
                if self.flag("lamp_lit") {
                    Some(Room::Cellar)
                } else {
                    return "It is pitch black down there. I need light.".to_string();
                }
            }
            (Room::Cellar, "up") => Some(Room::Library),
            (Room::Cellar, "east") => {
                if self.has("brass key") {
                    Some(Room::Vault)
                } else {
                    return "The vault door is locked; a brass keyhole waits.".to_string();
                }
            }
            (Room::Vault, "west") => Some(Room::Cellar),
            (Room::Garden, "north") => Some(Room::Hall),
            _ => None,
        };
        match next {
            Some(room) => {
                if room == Room::Vault && !self.flag("vault_seen") {
                    self.flags.insert("vault_seen".to_string());
                    self.room = room;
                    format!("{}\nYou found the treasure!", room.describe())
                } else {
                    self.room = room;
                    format!("{}\n{}", room.describe(), "You move carefully.")
                }
            }
            None => format!("You can't go {} from here.", direction),
        }
    }

    fn take(&mut self, item: &str) -> String {
        let present = match self.room {
            Room::Library => item == "brass key" || item == "old book" || item == "lamp",
            Room::Cellar => item == "lamp",
            Room::Garden => item == "rose",
            _ => false,
        };
        if !present {
            return format!("There is no {} here.", item);
        }
        if self.has(item) {
            return format!("You already have the {}.", item);
        }
        self.inventory.push(item.to_string());
        format!("Taken: {}", item)
    }

    fn use_item(&mut self, item: &str) -> String {
        if !self.has(item) {
            return format!("You aren't carrying a {}.", item);
        }
        match item {
            "lamp" => {
                self.flags.insert("lamp_lit".to_string());
                "The lamp flickers to life.".to_string()
            }
            "brass key" => {
                if self.room == Room::Cellar {
                    "The key fits the vault door.".to_string()
                } else {
                    "Nothing to unlock here.".to_string()
                }
            }
            "old book" => {
                self.flags.insert("read_book".to_string());
                "A map falls out: the vault lies east of the cellar.".to_string()
            }
            "rose" => {
                self.flags.insert("smelled_rose".to_string());
                "It smells faintly of summer.".to_string()
            }
            other => format!("The {} does nothing useful.", other),
        }
    }

    fn look(&self) -> String {
        let mut text = format!("== {} ==\n{}", self.room.title(), self.room.describe());
        let exits = match self.room {
            Room::Hall => "north, south",
            Room::Library => "south, down",
            Room::Cellar => "up, east",
            Room::Vault => "west",
            Room::Garden => "north",
        };
        text.push_str(&format!("\nExits: {}", exits));
        text
    }

    fn inventory_line(&self) -> String {
        if self.inventory.is_empty() {
            "You carry nothing.".to_string()
        } else {
            let mut items = self.inventory.clone();
            items.sort();
            format!("You carry: {}", items.join(", "))
        }
    }

    fn apply(&mut self, command: Command) -> String {
        self.moves += 1;
        match command {
            Command::Go(direction) => self.go(&direction),
            Command::Take(item) => self.take(&item),
            Command::Use(item) => self.use_item(&item),
            Command::Look => self.look(),
            Command::Inventory => self.inventory_line(),
            Command::Quit => {
                self.finished = true;
                "Farewell, adventurer.".to_string()
            }
        }
    }

    fn score(&self) -> i32 {
        let mut score = 0;
        score += self.inventory.len() as i32 * 5;
        score += self.flags.len() as i32 * 10;
        if self.flag("vault_seen") {
            score += 40;
        }
        score -= self.moves as i32 / 4;
        score
    }
}

fn main() {
    let script = [
        "look",
        "go north",
        "take brass key",
        "take old book",
        "take lamp",
        "take lamp",
        "inventory",
        "go down",
        "use lamp",
        "go down",
        "go east",
        "look",
        "go west",
        "go up",
        "go south",
        "take rose",
        "use rose",
        "use brass key",
        "go north",
        "go north",
        "go down",
        "go east",
        "dance",
        "",
        "inventory",
        "quit",
        "look",
    ];
    let mut game = Game::new();
    for line in script {
        if game.finished {
            break;
        }
        let command = match parse(line) {
            Ok(command) => command,
            Err(message) => {
                println!("> {}\n{}", line, message);
                continue;
            }
        };
        let output = game.apply(command);
        println!("> {}\n{}", line, output);
    }
    println!("---");
    println!("moves: {}, score: {}", game.moves, game.score());
    let mut flags: Vec<&String> = game.flags.iter().collect();
    flags.sort();
    for flag in flags {
        println!("flag: {}", flag);
    }
    println!("final room: {}", game.room);
    let collected: Vec<&str> = game.inventory.iter().map(|item| item.as_str()).collect();
    println!("collected: {:?}", collected);
}
