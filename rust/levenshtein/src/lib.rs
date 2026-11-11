pub fn levenshtein(a: &str, b: &str) -> usize {
    let mut result = 0;

    if a == b {
        return result;
    }

    let bytes_a = a.as_bytes();
    let bytes_b = b.as_bytes();
    let length_a = bytes_a.len();
    let length_b = bytes_b.len();

    if length_a == 0 {
        return length_b;
    }

    if length_b == 0 {
        return length_a;
    }

    let mut cache: Vec<usize> = vec![];
    for i in 1..=length_a {
        cache.push(i);
    }

    for (index_b, code_b) in bytes_b.iter().enumerate() {
        result = index_b;
        let mut distance_a = index_b;

        for (index_a, code_a) in bytes_a.iter().enumerate() {
            let distance_b = if code_a == code_b {
                distance_a
            } else {
                distance_a + 1
            };

            distance_a = cache[index_a];

            result = if distance_a > result {
                if distance_b > result {
                    result + 1
                } else {
                    distance_b
                }
            } else if distance_b > distance_a {
                distance_a + 1
            } else {
                distance_b
            };

            cache[index_a] = result;
        }
    }

    result
}

#[test]
fn test_levenshtein() {
    assert_eq!(0, levenshtein("kitten", "kitten"));
    assert_eq!(3, levenshtein("", "abc"));
    assert_eq!(3, levenshtein("abc", ""));
    assert_eq!(3, levenshtein("kitten", "sitting"));
    assert_eq!(2, levenshtein("flaw", "lawn"));
    assert_eq!(1, levenshtein("a", "b"));
}
