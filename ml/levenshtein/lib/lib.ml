let levenshtein a b =
    let exception Return of int in
    (try
        let result = ref 0 in
        if a = b then
            raise (Return !result);
        let bytes_a = a in
        let bytes_b = b in
        let length_a = String.length bytes_a in
        let length_b = String.length bytes_b in
        if length_a = 0 then
            raise (Return length_b);
        if length_b = 0 then
            raise (Return length_a);
        let cache = Dynarray.create () in
        for i = 1 to length_a do
            Dynarray.add_last cache i
        done;
        String.iteri (fun index_b code_b ->
            let code_b = Char.code code_b in
            result := index_b;
            let distance_a = ref index_b in
            String.iteri (fun index_a code_a ->
                let code_a = Char.code code_a in
                let distance_b =
                    if code_a = code_b then
                        !distance_a
                    else
                        !distance_a + 1
                in
                distance_a := Dynarray.get cache index_a;
                result := (if !distance_a > !result then (if distance_b > !result then !result + 1 else distance_b) else if distance_b > !distance_a then !distance_a + 1 else distance_b);
                Dynarray.set cache index_a !result) bytes_a) bytes_b;
        !result
    with Return r -> r)
