open Lib

let () =
    let steps = collatz 3 in
    assert (8 = Dynarray.length steps);
    assert (3 = Dynarray.get steps 0);
    assert (10 = Dynarray.get steps 1);
    assert (1 = Dynarray.get steps 7);
    assert (Some 1 = Dynarray.pop_last_opt steps);
    assert (7 = Dynarray.length steps)
