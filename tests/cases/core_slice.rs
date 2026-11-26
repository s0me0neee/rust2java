// Slice methods on Vec / array / byte-string receivers: chunking, splitting, searching,
// sorting, copying and the ASCII helpers of [u8].
fn main() {
    let v = vec![1, 2, 3, 4, 5, 6, 7];

    // chunks and windows from either end
    println!("{:?}", v.rchunks(3).collect::<Vec<_>>());
    println!("{:?} {:?}", v.rchunks_exact(3).collect::<Vec<_>>(), v.rchunks_exact(3).remainder());
    println!("{:?}", v.array_windows::<3>().map(|w| w[0] + w[1] + w[2]).collect::<Vec<_>>());
    let (chunks, rest) = v.as_chunks::<2>();
    println!("{:?} {:?}", chunks, rest);
    let (head, rchunks) = v.as_rchunks::<3>();
    println!("{:?} {:?}", head, rchunks);
    println!("{:?}", [1, 1, 2, 3, 3, 3, 1].chunk_by(|a, b| a == b).collect::<Vec<_>>());
    println!("{:?} {:?}", v.first_chunk::<2>(), v.last_chunk::<9>());
    println!("{:?} {:?}", v.split_first_chunk::<3>(), v.split_last_chunk::<2>());
    println!("{:?} {:?}", v.as_array::<7>(), v.as_array::<3>());
    let grid = [[1, 2], [3, 4], [5, 6]];
    println!("{:?}", grid.as_flattened());

    // splitting by predicate
    println!("{:?}", v.split(|x| x % 3 == 0).collect::<Vec<_>>());
    println!("{:?}", v.rsplit(|x| x % 3 == 0).collect::<Vec<_>>());
    println!("{:?} {:?}", v.splitn(2, |x| x % 2 == 0).collect::<Vec<_>>(), v.rsplitn(2, |x| x % 2 == 0).collect::<Vec<_>>());
    println!("{:?}", v.split_inclusive(|x| x % 3 == 0).collect::<Vec<_>>());
    println!("{:?} {:?}", v.split_at_checked(2), v.split_at_checked(8).is_none());

    // ends, prefixes, affixes
    println!("{} {} {}", v.is_empty(), v.ends_with(&[6, 7]), v.ends_with(&[5]));
    println!("{:?} {:?}", v.strip_prefix(&[1, 2]), v.strip_suffix(&[9]));
    println!("{:?} {:?}", v.strip_circumfix(&[1], &[7]), v.trim_prefix(&[1, 2, 3]));
    println!("{:?}", v.trim_suffix(&[7]));

    // searching and sorting
    let people = [("ann", 31), ("bob", 25), ("cy", 40)];
    let by_age: Vec<_> = {
        let mut p = people.to_vec();
        p.sort_by_cached_key(|(_, age)| *age);
        p
    };
    println!("{:?}", by_age);
    let ages: Vec<i32> = by_age.iter().map(|p| p.1).collect();
    println!("{:?} {:?}", by_age.binary_search_by_key(&31, |p| p.1), by_age.binary_search_by_key(&30, |p| p.1));
    println!("{} {} {}", ages.is_sorted(), [3, 1].is_sorted_by(|a, b| a >= b), people.is_sorted_by_key(|p| p.0));
    let mut w = vec![9, 1, 8, 2, 7, 3];
    let (_, mid, _) = w.select_nth_unstable_by(2, |a, b| b.cmp(a));
    println!("{}", mid);
    let (_, mid, _) = w.select_nth_unstable_by_key(1, |x| (*x as i32 - 5).abs());
    println!("{}", (*mid as i32 - 5).abs());

    // mutation through views
    let mut m = vec![0; 7];
    for (i, c) in m.chunks_mut(3).enumerate() {
        c[0] = i as i32 + 1;
    }
    for c in m.rchunks_mut(2) {
        c[c.len() - 1] += 10;
    }
    println!("{:?}", m);
    for c in m.chunks_exact_mut(2) {
        c.swap(0, 1);
    }
    for c in m.rchunks_exact_mut(3) {
        c.reverse();
    }
    println!("{:?}", m);
    for part in m.split_mut(|x| *x == 0) {
        part.fill(5);
    }
    println!("{:?}", m);
    // `&mut` items: Java shares objects (a `Vec`), not primitives, through them.
    let mut lists = vec![vec![1], vec![2], vec![3]];
    if let Some((first, rest)) = lists.split_first_mut() {
        first.push(rest.len() as i32);
    }
    if let Some((last, rest)) = lists.split_last_mut() {
        last.push(rest[0][0] * 100);
    }
    let [x, y] = lists.get_disjoint_mut([0, 2]).unwrap();
    x.push(7);
    y.push(8);
    let overlap = lists.get_disjoint_mut([1, 1]).is_err();
    println!("{:?} {}", lists, overlap);
    let mut n = vec![1, 2, 3, 4, 5];
    n[0] = n[1..].len() as i32;
    println!("{:?}", n);
    let mut counter = 0;
    n.fill_with(|| {
        counter += 2;
        counter
    });
    n.copy_within(0..2, 3);
    println!("{:?}", n);
    let mut other = vec![0; 5];
    n.swap_with_slice(&mut other);
    other.clone_from_slice(&[9, 9, 9, 9, 9]);
    println!("{:?} {:?}", n, other);
    n[0] = 7;
    n[2] = 8;
    println!("{:?}", n);
    for part in n.rsplit_mut(|v| *v == 8) {
        if let Some(f) = part.first_mut() {
            *f += 1;
        }
    }
    println!("{:?}", n);
    for part in n.splitn_mut(2, |v| *v == 0) {
        part.reverse();
    }
    for part in n.rsplitn_mut(2, |v| *v == 1) {
        part.sort();
    }
    for part in n.split_inclusive_mut(|v| *v == 0) {
        part.rotate_left(1);
    }
    println!("{:?}", n);
    for g in [1, 1, 2, 2, 2].to_vec().chunk_by_mut(|a, b| a == b) {
        g[0] = g.len() as i32;
        println!("{:?}", g);
    }
    if let Some(c) = n.first_chunk_mut::<2>() {
        c[1] = 42;
    }
    if let Some(c) = n.last_chunk_mut::<2>() {
        c[0] = 43;
    }
    if let Some((c, _)) = n.split_first_chunk_mut::<1>() {
        c[0] = 41;
    }
    if let Some((_, c)) = n.split_last_chunk_mut::<1>() {
        c[0] = 44;
    }
    println!("{:?}", n);
    if let Some(a) = n.as_mut_array::<5>() {
        a[2] = 0;
    }
    let (pairs, _) = n.as_chunks_mut::<2>();
    pairs[0][0] = -1;
    let (_, tail) = n.as_rchunks_mut::<2>();
    tail[0][1] = -2;
    println!("{:?}", n);
    let mut flat = [[0u8; 2]; 2];
    flat.as_flattened_mut()[3] = 1;
    println!("{:?}", flat);

    // removing from the ends of a slice reference
    let mut s: &[i32] = &[1, 2, 3, 4, 5];
    println!("{:?} {:?}", s.split_off_first(), s.split_off_last());
    println!("{:?} {:?}", s.split_off(..1), s);
    let mut cells = [vec![10], vec![20], vec![30], vec![40]];
    let mut ms: &mut [Vec<i32>] = &mut cells;
    if let Some(f) = ms.split_off_first_mut() {
        f.push(1);
    }
    if let Some(l) = ms.split_off_last_mut() {
        l.push(1);
    }
    println!("{:?} {:?}", ms.split_off_mut(1..), ms);
    println!("{:?}", cells);

    // [u8] and strings of slices
    let bytes = b"  Hello\tWorld! ";
    println!("{} {}", bytes.is_ascii(), bytes.eq_ignore_ascii_case(b"  hello\tworld! "));
    println!("{:?} {:?}", bytes.trim_ascii(), bytes.trim_ascii_start().len() + bytes.trim_ascii_end().len());
    println!("{:?} {:?}", bytes.to_ascii_uppercase(), b"ABC".to_ascii_lowercase());
    let mut word = *b"MiXeD";
    word.make_ascii_lowercase();
    let mut loud = *b"quiet";
    loud.make_ascii_uppercase();
    println!("{:?} {:?}", word, loud);
    println!("{} {}", b"a\tb\"\x7f".escape_ascii(), b"ok".escape_ascii().to_string().len());
    for chunk in b"ab\xffcd".utf8_chunks() {
        println!("{:?} {:?}", chunk.valid(), chunk.invalid());
    }
    println!("{:?} {:?}", [[1, 2], [3, 4]].concat(), [vec![1], vec![2, 3]].connect(&0));
    println!("{:?} {:?}", [1, 2].repeat(3), vec![4, 5].into_boxed_slice().into_vec());
}
