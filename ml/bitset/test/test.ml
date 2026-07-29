open Lib

let () =
    let b = with_capacity 16 in
    toggle b 1;
    ignore (put b 2);
    toggle b 2;
    ignore (put b 3);
    assert (contains b 1);
    assert (not (contains b 2));
    assert (contains b 3)
