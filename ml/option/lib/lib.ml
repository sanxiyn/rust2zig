module Option = struct
    type 't t =
        | Some of 't
        | None

    let and_ self optb =
        match self with
        | Some _ -> optb
        | None -> None

    let is_none self =
        match self with
        | Some _ -> false
        | None -> true

    let is_some self =
        match self with
        | Some _ -> true
        | None -> false

    let unwrap self =
        match self with
        | Some x -> x
        | None -> failwith "called unwrap on None"
end
