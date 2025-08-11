module account_auth_example::m;

use iota::account;
use std::ascii;

public fun crate_auth() {
    let account_info = account::create_auth_info_v1(@0x0, ascii::string(b"m"), ascii::string(b"authenticate"));
    account::drop_auth_info_v1(account_info);
}

public fun authenticate(): bool {
    true
}
