open Lib

let () =
    assert (0 = levenshtein "kitten" "kitten");
    assert (3 = levenshtein "" "abc");
    assert (3 = levenshtein "abc" "");
    assert (3 = levenshtein "kitten" "sitting");
    assert (2 = levenshtein "flaw" "lawn");
    assert (1 = levenshtein "a" "b")
