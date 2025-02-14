// options:
// printWidth: 40
// useModuleLabel: true
// autoGroupImports: module

module prettier::group_imports;

use a::b as c;
use a::b::e as f;
use a::{b::g as f, b::h as i};

use iota::balance::{Self, Balance};
use iota::coin::{Self, Coin};
use iota::dynamic_field as df;
use iota::dynamic_object_field as dof;
use iota::event;
use iota::iota::IOTA;
use iota::transfer_policy::{Self, TransferPolicy, TransferRequest};

use std::{
    string::String,
    ascii::String as ASCII,
    vector as vec,
    option::{Self as opt, Option},
    type_name::get as type_name_get,
};

public fun do_something() {}
