fn prefix_function(s: &[u8]) -> Vec<usize> {
    let mut pi = vec![0; s.len()];
    for i in 1..s.len() {
        let mut k = pi[i - 1];
        while k > 0 && s[i] != s[k] {
            k = pi[k - 1];
        }
        if s[i] == s[k] {
            k += 1;
        }
        pi[i] = k;
    }
    pi
}

fn find_all(text: &str, pat: &str) -> Vec<usize> {
    let combined = format!("{pat}#{text}");
    let pi = prefix_function(combined.as_bytes());
    pi.iter()
        .enumerate()
        .skip(pat.len() + 1)
        .filter(|&(_, &v)| v == pat.len())
        .map(|(i, _)| i - 2 * pat.len())
        .collect()
}

fn main() {
    println!("{:?}", prefix_function(b"aabaaab"));
    println!("{:?}", find_all("abababcab", "ab"));
    println!("{:?}", find_all("aaaa", "aa"));
    let s = "abcabcabc";
    let pi = prefix_function(s.as_bytes());
    let period = s.len() - pi[s.len() - 1];
    if s.len() % period == 0 {
        println!("period {} repeated {} times: {}", period, s.len() / period, &s[..period]);
    }
}
