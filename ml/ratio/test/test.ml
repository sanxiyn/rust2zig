open Lib

let () =
    let a = { num = 1; denom = 2 } in
    let b = { num = 1; denom = 3 } in
    let c = add a b in
    assert (5 = c.num);
    assert (6 = c.denom);
    let d = add b b in
    assert (2 = d.num);
    assert (3 = d.denom)
