open Lib

let () =
    let origin = { Point.x = 1; y = 2 } in
    let size = { Size.width = 3; height = 4 } in
    let rect = { Rect.origin; size } in
    assert (not (Rect.contains rect { Point.x = 0; y = 0 }));
    assert (not (Rect.contains rect { Point.x = 0; y = 3 }));
    assert (not (Rect.contains rect { Point.x = 0; y = 7 }));
    assert (not (Rect.contains rect { Point.x = 3; y = 0 }));
    assert (Rect.contains rect { Point.x = 3; y = 3 });
    assert (not (Rect.contains rect { Point.x = 3; y = 7 }));
    assert (not (Rect.contains rect { Point.x = 7; y = 0 }));
    assert (not (Rect.contains rect { Point.x = 7; y = 3 }));
    assert (not (Rect.contains rect { Point.x = 7; y = 7 }))
