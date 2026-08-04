open Lib

let () =
    let x = Option.Some 42 in
    let y = Option.None in
    assert (true = Option.is_some x);
    assert (false = Option.is_some y);
    assert (42 = Option.unwrap x);
    let z = Option.Some 7 in
    assert (true = Option.is_none (Option.and_ y z))
