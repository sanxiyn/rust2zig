let gcd _a _b =
    let a = ref _a in
    let b = ref _b in
    while !b <> 0 do
        let t = !b in
        b := !a mod !b;
        a := t
    done;
    !a
