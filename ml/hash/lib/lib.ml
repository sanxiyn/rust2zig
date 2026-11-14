let fnv_offset_basis_32 =
    0x811c9dc5l

let fnv_prime_32 =
    0x01000193l

let fnv1a_hash_32 bytes limit =
    let prime = fnv_prime_32 in
    let hash = ref fnv_offset_basis_32 in
    let i = ref 0 in
    let len =
        match limit with
        | Some v when 0 < v && v < String.length bytes -> v
        | _ -> String.length bytes
    in
    while !i < len do
        hash := Int32.logxor !hash (Int32.of_int (Char.code bytes.[!i]));
        hash := Int32.mul !hash prime;
        i := !i + 1
    done;
    !hash

let fnv1a_hash_str_32 input =
    fnv1a_hash_32 input None

let foobar =
    "foobar"

let foobar_hash_32 =
    0xbf9cf968l
