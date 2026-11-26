// char and str methods. Byte offsets are only exact on ASCII text (Java indexes UTF-16 units), so
// the offset-based calls use ASCII strings.
fn main() {
    // char classification
    let cs = ['a', 'Z', '7', ' ', '\n', 'é', '٣', '!', '\u{7f}', '~'];
    for c in cs {
        println!(
            "{:?}: {} {} {} {} {} {} {} {} {} {}",
            c,
            c.is_ascii(),
            c.is_ascii_alphabetic(),
            c.is_ascii_alphanumeric(),
            c.is_ascii_control(),
            c.is_ascii_graphic(),
            c.is_control(),
            c.is_lowercase(),
            c.is_numeric(),
            c.is_digit(16),
            c.eq_ignore_ascii_case(&'A'),
        );
    }
    // case and encodings
    println!("{} {} {} {}", 'é'.to_ascii_uppercase(), 'é'.to_uppercase(), 'ß'.to_uppercase(), 'Σ'.to_lowercase());
    println!("{} {}", 'a'.to_uppercase().collect::<String>(), 'Q'.to_lowercase().next().unwrap());
    let mut m = 'Q';
    m.make_ascii_lowercase();
    println!("{m} {} {} {} {}", 'a'.len_utf8(), 'é'.len_utf8(), '€'.len_utf8(), '€'.len_utf16());
    let mut buf = [0u8; 4];
    let s = 'é'.encode_utf8(&mut buf);
    let mut ascii = [0u8; 4];
    println!("{} {}", s, 'z'.encode_utf8(&mut ascii).len());
    let mut wide = [0u16; 2];
    println!("{:?}", '€'.encode_utf16(&mut wide));
    let decoded: String = char::decode_utf16([0x48u16, 0x69, 0x20ac]).map(|r| r.unwrap_or('?')).collect();
    println!("{}", decoded);
    println!("{} {} {}", '\n'.escape_debug(), '"'.escape_default(), 'é'.escape_unicode());
    println!("{} {}", '\''.escape_debug(), 'a'.escape_default());

    // str: ASCII-only checks and case
    let text = "Hello, World";
    println!("{} {} {}", text.is_ascii(), "héllo".is_ascii(), text.to_ascii_uppercase());
    println!("{} {}", "ÉcOLE".to_ascii_lowercase(), "straße".to_ascii_uppercase());
    let mut owned = String::from("MiXeD");
    owned.make_ascii_lowercase();
    let mut loud = String::from("quiet");
    loud.make_ascii_uppercase();
    println!("{owned} {loud}");
    println!("[{}] [{}] [{}]", "  pad \t".trim_ascii(), "  pad ".trim_ascii_start(), "  pad ".trim_ascii_end());

    // str: matching
    let csv = "a,b,,c,";
    println!("{:?}", csv.match_indices(',').collect::<Vec<_>>());
    println!("{:?}", "abcabc".rmatch_indices("bc").collect::<Vec<_>>());
    println!("{:?}", "a1b22c333".rmatches(char::is_numeric).collect::<Vec<_>>());
    println!("{:?} {:?}", "x;y;z;".rsplit_terminator(';').collect::<Vec<_>>(), "line1\nline2\n".split_inclusive('\n').collect::<Vec<_>>());
    println!("{} {} {}", "xxabcxx".trim_start_matches('x'), "xxabcxx".trim_end_matches('x'), "12ab34".trim_end_matches(char::is_numeric));
    println!("{} {}", "--opt".trim_prefix("--"), "file.rs".trim_suffix(".txt"));
    println!("{:?} {:?}", "[tag]".strip_circumfix("[", "]"), "[tag".strip_circumfix("[", "]"));

    // str: offsets (ASCII)
    let word = "keyboard";
    let (l, r) = word.split_at(3);
    println!("{l} {r} {:?} {:?}", word.split_at_checked(5), word.split_at_checked(9));
    // `&mut str` views are read here: Java can't write through them into the String.
    let mut name = String::from("johnsmith");
    {
        let (first, last) = name.split_at_mut(4);
        println!("{first} {last}");
    }
    println!("{:?} {:?}", name.split_at_mut_checked(20).is_none(), word.is_char_boundary(3));
    println!("{} {} {}", word.floor_char_boundary(5), word.ceil_char_boundary(5), word.floor_char_boundary(99));
    let mut boxed = String::from("abcdef");
    println!("{:?}", boxed.get_mut(1..3).map(|part| part.len()));

    // str: encodings and escapes
    println!("{:?}", "hé€".encode_utf16().collect::<Vec<u16>>());
    println!("{} | {} | {}", "tab\t\"q\" é".escape_debug(), "tab\t\"q\" é".escape_default(), "hi".escape_unicode());
    let b: Box<str> = "boxed".into();
    println!("{} {:?}", b.clone().into_string(), b.into_boxed_bytes());
    let mut raw = *b"hi there";
    let fixed = std::str::from_utf8_mut(&mut raw).unwrap();
    println!("{} {}", fixed, "abc".trim_right_matches('c'));
}
