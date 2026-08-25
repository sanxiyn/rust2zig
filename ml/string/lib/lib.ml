let char_at pattern i =
    Uchar.utf_decode_uchar (String.get_utf_8_uchar pattern i)

let is_eof pattern offset =
    offset = String.length pattern

let bump pattern offset =
    offset + Uchar.utf_8_byte_length (char_at pattern offset)

let count_chars pattern =
    let offset = ref 0 in
    let count = ref 0 in
    while not (is_eof pattern !offset) do
        offset := bump pattern !offset;
        count := !count + 1
    done;
    !count

let count_lines pattern =
    let offset = ref 0 in
    let line = ref 1 in
    while not (is_eof pattern !offset) do
        if char_at pattern !offset = Uchar.of_char '\n' then
            line := !line + 1;
        offset := bump pattern !offset
    done;
    !line

let pattern =
    "a√\nz"
