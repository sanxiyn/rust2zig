open Lib

let () =
    assert (3 = Result.get_ok (eval 1 2 1));
    assert (Error DivideByZero = eval 1 2 0);
    assert (Error Overflow = eval 600 600 1)

let () =
    assert (3 = Result.get_ok (eval_chain 1 2 1));
    assert (Error DivideByZero = eval_chain 1 2 0)

let () =
    assert (3 = eval_or 1 2 1 0);
    assert (0 = eval_or 1 2 0 0)

let () =
    assert (Some 2 = quarter 8);
    assert (None = quarter 6)
