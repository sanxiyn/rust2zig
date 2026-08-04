open Lib

let () =
    let p = { Point.x = 1; y = 2 } in
    Point.translate p 3 4;
    assert (4 = p.x);
    assert (6 = p.y)

let () =
    let p = { Point.x = 1; y = 2 } in
    let (x0, y0, x1, y1) = bounding_box (Shape.Dot p) in
    assert (1 = x0);
    assert (2 = y0);
    assert (1 = x1);
    assert (2 = y1)

let () =
    let p = { Point.x = 1; y = 2 } in
    let q = { Point.x = 2; y = 1 } in
    let (x0, y0, x1, y1) = bounding_box (Shape.Line (p, q)) in
    assert (1 = x0);
    assert (1 = y0);
    assert (2 = x1);
    assert (2 = y1)

let () =
    let p = { Point.x = 2; y = 2 } in
    let (x0, y0, x1, y1) = bounding_box (Shape.Circle { center = p; radius = 1 }) in
    assert (1 = x0);
    assert (1 = y0);
    assert (3 = x1);
    assert (3 = y1)
