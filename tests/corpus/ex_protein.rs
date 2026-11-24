fn codon(c: &str) -> Option<&'static str> {
    Some(match c {
        "AUG" => "Methionine",
        "UUU" | "UUC" => "Phenylalanine",
        "UUA" | "UUG" => "Leucine",
        "UCU" | "UCC" | "UCA" | "UCG" => "Serine",
        "UAU" | "UAC" => "Tyrosine",
        "UGU" | "UGC" => "Cysteine",
        "UGG" => "Tryptophan",
        "UAA" | "UAG" | "UGA" => "STOP",
        _ => return None,
    })
}

fn proteins(rna: &str) -> Option<Vec<&'static str>> {
    let mut out = Vec::new();
    let bytes = rna.as_bytes();
    for chunk in bytes.chunks(3) {
        let name = codon(std::str::from_utf8(chunk).ok()?)?;
        if name == "STOP" {
            break;
        }
        out.push(name);
    }
    Some(out)
}

fn to_rna(dna: &str) -> Result<String, usize> {
    dna.chars()
        .enumerate()
        .map(|(i, c)| match c {
            'G' => Ok('C'),
            'C' => Ok('G'),
            'T' => Ok('A'),
            'A' => Ok('U'),
            _ => Err(i),
        })
        .collect()
}

fn main() {
    println!("{:?}", proteins("AUGUUUUCUUAAAUG"));
    println!("{:?}", proteins("UGGUGUUAUUAAUGGUUU"));
    println!("{:?}", proteins("AUGXYZ"));
    println!("{:?}", proteins("AUGU"));
    println!("{:?} {:?}", to_rna("ACGTGGTCTTAA"), to_rna("ACGTXXXCTTAA"));
}
