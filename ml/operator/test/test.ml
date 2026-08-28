open Lib

let () =
    assert (false = not_bool true);
    assert (true = not_bool false);
    assert (0xf0 = not_int 0x0f);
    assert (255 = not_int 0)
