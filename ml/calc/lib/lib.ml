module Error = struct
    type t =
        | Overflow
        | DivideByZero
end

let limit =
    1000

let add a b =
    let exception Return of (int, Error.t) result in
    (try
        let sum = a + b in
        if sum > limit then
            raise (Return (Error Error.Overflow));
        Ok sum
    with Return r -> r)

let div a b =
    let exception Return of (int, Error.t) result in
    (try
        if b = 0 then
            raise (Return (Error Error.DivideByZero));
        Ok (a / b)
    with Return r -> r)

let eval a b c =
    let ( let* ) = Result.bind in
    let* sum = add a b in
    div sum c

let eval_chain a b c =
    let ( let* ) = Result.bind in
    let* try1 = add a b in
    div try1 c

let eval_or a b c default =
    match eval a b c with
    | Ok value -> value
    | Error _ -> default

let half x =
    if x mod 2 = 0 then
        Some (x / 2)
    else
        None

let quarter x =
    let ( let* ) = Option.bind in
    let* h = half x in
    half h
