type ('t, 'e) result =
    | Ok of 't
    | Err of 'e

let is_ok self =
    match self with
    | Ok _ -> true
    | Err _ -> false

let is_err self =
    match self with
    | Ok _ -> false
    | Err _ -> true
