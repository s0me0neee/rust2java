// `.map(Command::Remove)`: the variant constructor is typed as producing the enum.
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
