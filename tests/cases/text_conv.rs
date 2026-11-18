// Conversions: char into String, String + &str, byte literals/strings, ascii predicates.
fn main() {
    println!("{}", String::from('q'));
    let owned: String = 'x'.into();
    println!("{}", owned + &"y".to_string());

    let mut s = String::from("ab");
    s += "cd";
    println!("{} {}", s, s.len());

    // Slicing uses Java char offsets, which equal Rust's byte offsets for ASCII.
    let ascii = "abcdef";
    println!("{} {} {}", &ascii[1..3], &ascii[4..], &ascii[..2]);

    // For non-ASCII, Rust cuts at byte offsets and we cut at char offsets.
    let text = "héllo";
    println!("{} {}", &text[..1], text.chars().count());

    println!("{} {}", (b'a' + 1) as char, b'z' as u32);
    println!("{} {} {}", b'x', b'x' + 1, (b'x' + 2) as char);
    println!("{:?} {:?}", b"ab", b"");
    println!("{}", char::from(65u8));
    println!("{} {}", '!'.is_ascii_punctuation(), 'a'.is_ascii_punctuation());
    println!("{:?} {:?}", 'a'.to_digit(16), 'f'.to_digit(16));

    let byte = 98u8;
    println!("{}", char::from(byte));

    let digits: Vec<Option<u32>> = "1aZ".chars().map(|c| c.to_digit(10)).collect();
    println!("{:?}", digits);
    println!("{:?}", char::from_u32(97));

    let raw = [104u8, 105];
    println!("{:?}", str::from_utf8(&raw).unwrap());
    println!("{}", str::from_utf8("hi".as_bytes()).unwrap());
}
