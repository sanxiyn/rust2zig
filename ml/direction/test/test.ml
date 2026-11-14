open Lib

let () =
    assert (Direction.South = opposite Direction.North);
    assert (Direction.West = opposite Direction.East);
    assert (Direction.North = opposite Direction.South);
    assert (Direction.East = opposite Direction.West)

let () =
    assert (true = vertical Direction.North);
    assert (false = vertical Direction.East);
    assert (true = vertical Direction.South);
    assert (false = vertical Direction.West)
