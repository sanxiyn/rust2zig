let () =
    let x = 3 in
    let double x = x * 2 in
    assert (6 = double x)

let () =
    let a = 3 in
    let add x = x + a in
    assert (6 = add 3)
