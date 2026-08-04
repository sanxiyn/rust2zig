module Direction = struct
    type t =
        | North
        | East
        | South
        | West
end

let opposite d =
    match d with
    | Direction.North -> Direction.South
    | Direction.East -> Direction.West
    | Direction.South -> Direction.North
    | Direction.West -> Direction.East
