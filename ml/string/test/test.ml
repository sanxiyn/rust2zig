open Lib

let () =
    assert (Uchar.of_char 'a' = char_at pattern 0);
    assert (Uchar.of_int 0x221A = char_at pattern 1);
    assert (Uchar.of_char '\n' = char_at pattern 4);
    assert (Uchar.of_char 'z' = char_at pattern 5)

let () =
    assert (not (is_eof pattern 0));
    assert (is_eof pattern 6)

let () =
    assert (1 = Uchar.utf_8_byte_length (char_at pattern 0));
    assert (3 = Uchar.utf_8_byte_length (char_at pattern 1))

let () =
    assert (4 = count_chars pattern)

let () =
    assert (2 = count_lines pattern)
