let dot a b =
    let sum = ref 0 in
    Array.iter2 (fun x y -> sum := !sum + x * y) a b;
    !sum
