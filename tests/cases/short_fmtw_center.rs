fn main() {
    println!("[{:^6}]", "ab");
    println!("[{:^6}]", "abcdefgh");
    println!("[{:^5}]", "mid");
    let w = 9;
    println!("[{:^w$}] [{:^w$}]", "hi", 7);
    println!("[{:*^7}] [{:*^7}] [{:-^7}]", "ab", -3, 1.5);
    println!("[{:.2}] [{:^8.2}] [{:^8.2}]", "abcdef", 1.234, -1.5);
    println!("[{:^8.3}] [{:^6}]", 1.25, "");
    let s = String::from("xy");
    println!("[{:^7}] [{:^7}] [{:^6}]", s, s, "");
    println!("{:^7?} {:^7?}", vec![1, 2], vec![Some(3), None]);
}
