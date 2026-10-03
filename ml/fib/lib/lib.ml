let fib n =
    let (a, b) = (1, 1) in
    let a = ref a and b = ref b in
    for _ = 2 to n do
        let (tmp1, tmp2) = (!b, !a + !b) in
        a := tmp1;
        b := tmp2
    done;
    !a
