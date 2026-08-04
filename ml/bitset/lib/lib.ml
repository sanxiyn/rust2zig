module BitSet = struct
    type t = {
        mutable data : int;
        length : int;
    }

    let with_capacity bits =
        { data = 0; length = bits }

    let contains self bit =
        bit < self.length && self.data land 1 lsl bit <> 0

    let put self bit =
        assert (bit < self.length);
        let prev = self.data land 1 lsl bit <> 0 in
        self.data <- self.data lor 1 lsl bit;
        prev

    let toggle self bit =
        assert (bit < self.length);
        self.data <- self.data lxor 1 lsl bit
end
