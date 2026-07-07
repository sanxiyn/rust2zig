open Lib

let () =
    let a = [| 1; 2; 3 |] in
    let b = [| 4; 5; 6 |] in
    assert (32 = dot a b)
