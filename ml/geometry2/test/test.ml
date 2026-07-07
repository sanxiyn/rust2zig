open Lib

let () =
    let p = { x = 1; y = 2 } in
    translate p 3 4;
    assert (4 = p.x);
    assert (6 = p.y)

let () =
    let p = { x = 1; y = 2 } in
    let (x0, y0, x1, y1) = bounding_box (Dot p) in
    assert (1 = x0);
    assert (2 = y0);
    assert (1 = x1);
    assert (2 = y1)

let () =
    let p = { x = 1; y = 2 } in
    let q = { x = 2; y = 1 } in
    let (x0, y0, x1, y1) = bounding_box (Line (p, q)) in
    assert (1 = x0);
    assert (1 = y0);
    assert (2 = x1);
    assert (2 = y1)

let () =
    let p = { x = 2; y = 2 } in
    let (x0, y0, x1, y1) = bounding_box (Circle { center = p; radius = 1 }) in
    assert (1 = x0);
    assert (1 = y0);
    assert (3 = x1);
    assert (3 = y1)
