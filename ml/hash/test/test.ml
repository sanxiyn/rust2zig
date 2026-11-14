open Lib

let () =
    let hashed = fnv1a_hash_str_32 foobar in
    assert (foobar_hash_32 = hashed)
