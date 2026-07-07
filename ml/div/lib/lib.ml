let div a b =
    if a mod b = 0 then
        Some (a / b)
    else
        None

let div2 a b =
    match div a b with
    | Some x -> x
    | None -> 0
