type direction =
    | North
    | East
    | South
    | West

let opposite d =
    match d with
    | North -> South
    | East -> West
    | South -> North
    | West -> East
