open Lib

let () =
    let x = Ok 42 in
    let y = Err 1 in
    assert (true = is_ok x);
    assert (true = is_err y)
