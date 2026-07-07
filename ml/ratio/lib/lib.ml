let gcd _a _b =
    let a = ref _a in
    let b = ref _b in
    while !b <> 0 do
        let t = !b in
        b := !a mod !b;
        a := t
    done;
    !a

type ratio = {
    num : int;
    denom : int;
}

let add self other =
    let n = self.num * other.denom + other.num * self.denom in
    let d = self.denom * other.denom in
    let g = gcd n d in
    { num = n / g; denom = d / g }
