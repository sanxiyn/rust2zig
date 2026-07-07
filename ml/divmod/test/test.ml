open Lib

let () =
    let (q, r) = divmod 7 3 in
    assert (2 = q);
    assert (1 = r)
