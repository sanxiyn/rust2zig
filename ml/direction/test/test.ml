open Lib

let () =
    assert (South = opposite North);
    assert (West = opposite East);
    assert (North = opposite South);
    assert (East = opposite West)
