open Lib

let () =
    let three = Nat.succ (Nat.succ (Nat.succ Nat.Zero)) in
    assert (3 = Nat.to_int three)
