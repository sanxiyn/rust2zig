type point = {
    x : int;
    y : int;
}

let translate self dx dy =
    { x = self.x + dx; y = self.y + dy }

type shape =
    | Dot of point
    | Line of point * point
    | Circle of { center : point; radius : int }

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
    | Dot p -> (p.x, p.y, p.x, p.y)
    | Line (p, q) -> (min p.x q.x, min p.y q.y, max p.x q.x, max p.y q.y)
    | Circle { center; radius } -> (center.x - radius, center.y - radius, center.x + radius, center.y + radius)
