open Lib

let () =
    assert (Direction.South = opposite Direction.North);
    assert (Direction.West = opposite Direction.East);
    assert (Direction.North = opposite Direction.South);
    assert (Direction.East = opposite Direction.West)
