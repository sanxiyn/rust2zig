open Lib

let () =
    let xs = [| 1; 2; 3; 4; 5 |] in
    let total = ref 0 in
    Array.iter (fun x -> total := !total + x) xs;
    assert (15 = !total);
    assert (15 = sum xs);
    total := 0;
    for x = 1 to 5 do
        total := !total + x
    done;
    assert (15 = !total)

let () =
    let xs = [| 1; 2; 3; 4; 5 |] in
    assert (15 = sum2 xs)

let () =
    let xs = [| 1; 2; 3; 4; 5 |] in
    assert (9 = sum_odd xs)
