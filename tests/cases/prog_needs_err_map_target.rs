// `Result::map` does not take the surrounding expected type, so the mapped Ok type is lost.
#[derive(Debug)]
enum Command {
    Remove(usize),
}

fn parse(s: &str) -> Result<Command, String> {
    s.parse::<usize>()
        .map(Command::Remove)
        .map_err(|e| e.to_string())
}

fn main() {
    println!("{:?}", parse("5"));
    println!("{:?}", parse("x"));
}
