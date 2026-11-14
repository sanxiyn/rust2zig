module Rand32 = struct
    type t = {
        mutable state : int64;
        inc : int64;
    }

    let default_inc =
        1442695040888963407L

    let multiplier =
        6364136223846793005L

    let rand_u32 self =
        let oldstate = self.state in
        self.state <- Int64.add (Int64.mul oldstate multiplier) self.inc;
        let xorshifted = Int64.to_int (Int64.shift_right_logical (Int64.logxor (Int64.shift_right_logical oldstate 18) oldstate) 27) land 0xffffffff in
        let rot = Int64.to_int (Int64.shift_right_logical oldstate 59) land 0xffffffff in
        xorshifted lsr rot lor xorshifted lsl (32 - rot) land 0xffffffff

    let new_inc seed increment =
        let rng = { state = 0L; inc = Int64.logor (Int64.shift_left increment 1) 1L } in
        let _ = rand_u32 rng in
        rng.state <- Int64.add rng.state seed;
        let _ = rand_u32 rng in
        rng

    let new_ seed =
        new_inc seed default_inc
end
