let collatz start =
    let steps = Dynarray.create () in
    let n = ref start in
    (try
        while true do
            Dynarray.add_last steps !n;
            if !n = 1 then
                raise Exit;
            match !n mod 2 with
            | 0 -> n := !n / 2
            | 1 -> n := 3 * !n + 1
            | _ -> failwith "unreachable"
        done
    with Exit -> ());
    steps
