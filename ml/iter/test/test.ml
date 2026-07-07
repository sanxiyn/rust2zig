open Lib

let () =
    let l = [| 1; 2; 3; 4; 5 |] in
    let v = 3 in
    assert (Some 2 = position l v);
    let v = 6 in
    assert (None = position l v)

let () =
    let l = [| 1; 2; 3; 4; 5 |] in
    let v = 3 in
    assert (Some 2 = position2 l v);
    let v = 6 in
    assert (None = position2 l v)
