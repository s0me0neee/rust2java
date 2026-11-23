use std::io::stdin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut lines = stdin().lines();
    let t = lines.next().unwrap()?.parse::<usize>()?;
    for _ in 0..t {
        let l = lines.next().unwrap()?;
        let mut lr = l.split(",").collect::<Vec<_>>();
        let n = lr[0];
        lr.remove(0);
        let mut t: i32 = 0;
        lr.iter().for_each(|r| {
            let mut a = r.split(":");
            let h = a.next().unwrap();
            let m = a.next().unwrap();
            t += m.parse::<i32>().unwrap();
            t += h.parse::<i32>().unwrap() * 60
        });
        print!("{n}=");
        let h = t / 60;
        let m = t % 60;
        print!("{h} {} ", { if h == 1 { "hour" } else { "hours" } });
        if m == 0 {
            println!();
            continue;
        }
        println!("{m} {}", { if m == 1 { "mintue" } else { "mintues" } });
    }
    Ok(())
}
