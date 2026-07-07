open Lib

let () =
    assert (Some 2 = div 6 3);
    assert (None = div 7 3)

let () =
    assert (2 = div2 6 3);
    assert (0 = div2 7 3)
