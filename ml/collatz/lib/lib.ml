let collatz start =
    let steps = Dynarray.create () in
    let n = ref start in
    (try
        while true do
            Dynarray.add_last steps !n;
            if !n = 1 then
                raise Exit
            else
                if !n mod 2 = 0 then
                    n := !n / 2
                else
                    n := 3 * !n + 1
        done
    with Exit -> ());
    steps
