open Lib

let () =
    let b = BitSet.with_capacity 16 in
    BitSet.toggle b 1;
    ignore (BitSet.put b 2);
    BitSet.toggle b 2;
    ignore (BitSet.put b 3);
    assert (BitSet.contains b 1);
    assert (not (BitSet.contains b 2));
    assert (BitSet.contains b 3)
