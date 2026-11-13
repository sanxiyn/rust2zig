module Point = struct
    type t = {
        x : int;
        y : int;
    }
end

module Size = struct
    type t = {
        width : int;
        height : int;
    }
end

module Box = struct
    type t = {
        min : Point.t;
        max : Point.t;
    }

    let contains self (p:Point.t) =
        ((self.min.x <= p.x && p.x < self.max.x) && self.min.y <= p.y) && p.y < self.max.y
end

module Rect = struct
    type t = {
        origin : Point.t;
        size : Size.t;
    }

    let min self =
        let x = self.origin.x in
        let y = self.origin.y in
        { Point.x; y }

    let max self =
        let x = self.origin.x + self.size.width in
        let y = self.origin.y + self.size.height in
        { Point.x; y }

    let to_box self =
        let min = min self in
        let max = max self in
        { Box.min; max }

    let contains self p =
        Box.contains (to_box self) p
end
