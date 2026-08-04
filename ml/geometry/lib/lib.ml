module Point = struct
    type t = {
        x : int;
        y : int;
    }

    let translate self dx dy =
        { x = self.x + dx; y = self.y + dy }
end

module Shape = struct
    type t =
        | Dot of Point.t
        | Line of Point.t * Point.t
        | Circle of { center : Point.t; radius : int }
end

let min a b =
    if a < b then
        a
    else
        b

let max a b =
    if a > b then
        a
    else
        b

let bounding_box s =
    match s with
    | Shape.Dot p -> (p.x, p.y, p.x, p.y)
    | Shape.Line (p, q) -> (min p.x q.x, min p.y q.y, max p.x q.x, max p.y q.y)
    | Shape.Circle { center; radius } -> (center.x - radius, center.y - radius, center.x + radius, center.y + radius)
