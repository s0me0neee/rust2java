use std::env;
use std::io::{self, BufWriter, Write};

fn main() -> io::Result<()> {
    {
        let stdout = io::stdout();
        let mut lock = stdout.lock();
        write!(lock, "locked").unwrap();
        writeln!(lock, " {}", 7)?;
        lock.flush()?;
    }
    {
        let mut output = BufWriter::new(io::stdout());
        write!(output, "buffered ")?;
        writeln!(output, "{}", 9).unwrap();
        output.flush()?;
        writeln!(output, "flushed on drop")?;
    }
    writeln!(io::stderr(), "stderr marker").unwrap();
    io::stderr().flush()?;
    let first = env::args().nth(1).unwrap_or(String::from("no argument"));
    println!("arg={}", first);
    let rest: Vec<String> = env::args().skip(1).collect();
    println!("remaining={:?}", rest);
    let value = env::var("RUST2JAVA_W12_MISSING_82E274DA").unwrap_or(String::from("fallback"));
    println!("env={}", value);
    Ok(())
}
