fn main() {
    let word = "abc";
    let n = 42;
    let neg = -42;
    let f = 1.25f64;
    // Center and custom fill keep the concatenation path, where each piece is its own format.
    println!("[{:^7}] [{:*<7}] [{:*>7}]", word, word, word);
    println!("[{:^7}] [{:<6}] [{:>6}] [{:06}]", n, n, n, n);
    println!("[{:^7}] [{:<6}] [{:>6}] [{:06}] [{:05}]", neg, neg, neg, neg, word);
    println!("[{:^9}] [{:<9}] [{:>9}] [{:09}]", f, f, f, f);
    println!("[{:^7}] [{:<7.2}] [{:>7.2}] [{:7.2}] [{:.2}]", f, f, f, f, word);
    println!("[{:^6}] [{:6}] [{:<6}] [{:06}] [{:<6}]", 'x', 'x', true, true, "hi");
    println!("[{:^6}] [{:8}] [{:>8}] [{:08}]", word, 1.5, -1.5, 1.5);
    let w = 8;
    let p = 3;
    println!("[{:^6}] [{:>w$}] [{:<w$.p$}] [{:w$}]", n, f, f, word);
    // Alternate Display is ignored by Rust; it must not become a Java `%#d`.
    println!("[{:#}] [{:#}] [{:#}]", n, f, word);
    let u: u32 = 4000000000;
    println!("[{:^6}] [{:<12}] [{:>12}] [{:+12}]", u, u, u, u);
    let u16v: u16 = 65535;
    println!("[{:^6}] [{:<7}] [{:>7}]", u16v, u16v, u16v);
    let x = 7;
    println!("[{:^6}] [{:>8?}] [{:5?}] [{:05?}] [{:+?}]", x, x, x, x, x);
    println!("[{:^6}] [{:5?}] [{:05?}] [{:>5?}]", true, true, true, true);
    println!("[{:^6}] [{:>8?}] [{:5?}]", 1.0, 1.0, 1.0);
}
