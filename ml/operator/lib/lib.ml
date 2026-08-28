let not_bool b =
    not b

let not_int x =
    lnot x land 0xff
