open Lib

let () =
    let seed = 54321L in
    let r1 = Rand32.new_ seed in
    assert (2891073575 = Rand32.rand_u32 r1)
