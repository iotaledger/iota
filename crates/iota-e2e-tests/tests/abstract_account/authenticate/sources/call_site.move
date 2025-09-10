module authenticate::call_site;

use iota::account;
use std::ascii;

// Temporary intermediate function until AuthenticatorInfoV1 can be converted to BSC.
// Until then, a function which may return it as a value can't be called by the user.
public fun call_create_auth_info_using(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
) {
    let _ = account::create_auth_info_v1(package, module_name, function_name);
}
