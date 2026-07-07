let position l v =
    let i = ref 0 in
    (try
        Array.iter (fun e -> if e = v then raise Exit else i := !i + 1) l
    with Exit -> ());
    if !i = Array.length l then
        None
    else
        Some !i

let position2 l v =
    let exception Return of int option in
    (try
        Array.iteri (fun i e -> if e = v then raise (Return (Some i))) l;
        None
    with Return r -> r)
