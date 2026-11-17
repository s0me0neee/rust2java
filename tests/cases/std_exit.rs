// process::exit keeps the exit code (§9.3).
fn main() {
    println!("before exit");
    if 1 + 1 == 2 {
        std::process::exit(3);
    }
    println!("unreachable");
}
