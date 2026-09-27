open Lib

let () =
    let p = { Point.x = 1; y = 2 } in
    let { Point.x; y } = p in
    let x = ref x in
    x := !x + 3;
    let y = y + 4 in
    assert (4 = !x);
    assert (6 = y)
