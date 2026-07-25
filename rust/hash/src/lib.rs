const FNV_OFFSET_BASIS_32: u32 = 0x811c9dc5;
const FNV_PRIME_32: u32 = 0x01000193;

pub const fn fnv1a_hash_32(bytes: &[u8], limit: Option<usize>) -> u32 {
    let prime = FNV_PRIME_32;
    let mut hash = FNV_OFFSET_BASIS_32;
    let mut i = 0;
    let len = match limit {
        Some(v) if 0 < v && v < bytes.len() => {
            v
        },
        _ => {
            bytes.len()
        }
    };
    while i < len {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(prime);
        i += 1;
    }
    hash
}

pub const fn fnv1a_hash_str_32(input: &str) -> u32 {
    fnv1a_hash_32(input.as_bytes(), None)
}

const FOOBAR: &str = "foobar";
const FOOBAR_HASH_32: u32 = 0xbf9cf968;

#[test]
fn test_32() {
    let hashed = fnv1a_hash_str_32(FOOBAR);
    assert_eq!(FOOBAR_HASH_32, hashed);
}
