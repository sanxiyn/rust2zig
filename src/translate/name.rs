pub fn camel_to_snake(s: &str) -> String {
    let mut result: String = Default::default();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

pub fn escape_zig(name: &str) -> String {
    const ZIG_KEYWORDS: &[&str] = &[
        "addrspace", "align", "allowzero", "and", "anyframe", "anytype",
        "asm", "break", "callconv", "catch", "comptime", "const", "continue",
        "defer", "else", "enum", "errdefer", "error", "export", "extern",
        "fn", "for", "if", "inline", "linksection", "noalias", "noinline",
        "nosuspend", "opaque", "or", "orelse", "packed", "pub", "resume",
        "return", "struct", "suspend", "switch", "test", "threadlocal",
        "try", "union", "unreachable", "var", "volatile", "while",
    ];
    if ZIG_KEYWORDS.contains(&name) {
        format!("@\"{}\"", name)
    } else {
        name.to_string()
    }
}

pub fn snake_to_camel(s: &str) -> String {
    let mut result: String = Default::default();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}
