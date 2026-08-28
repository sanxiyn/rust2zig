module Nat = struct
    type t =
        | Zero
        | Succ of t

    let succ n =
        Succ n

    let rec to_int self =
        match self with
        | Zero -> 0
        | Succ n -> 1 + to_int n
end
