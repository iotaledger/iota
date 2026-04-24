module a::m;

use std::ascii::{String, char};

#[view]
public fun update_string_by_value(mut name: String): String {
    name.push_char(char(43));
    name
}
