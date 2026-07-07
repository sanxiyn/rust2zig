let sum xs =
    let total = ref 0 in
    Array.iter (fun x -> total := !total + x) xs;
    !total

let sum2 xs =
    let total = ref 0 in
    for i = 0 to Array.length xs - 1 do
        total := !total + xs.(i)
    done;
    !total

let sum_odd xs =
    let total = ref 0 in
    Array.iter (fun x -> if x mod 2 = 0 then () else total := !total + x) xs;
    !total
