let rec fib n =
    let exception Return of int in
    (try
        if n < 2 then
            raise (Return n);
        fib (n - 1) + fib (n - 2)
    with Return r -> r)

let rec is_even n =
    let exception Return of bool in
    (try
        if n = 0 then
            raise (Return true);
        is_odd (n - 1)
    with Return r -> r)

and is_odd n =
    let exception Return of bool in
    (try
        if n = 0 then
            raise (Return false);
        is_even (n - 1)
    with Return r -> r)
