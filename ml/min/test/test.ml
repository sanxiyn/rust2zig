open Lib

let () =
    let a = 2 in
    let b = 3 in
    assert (2 = min a b);
    assert (2 = min b a)
