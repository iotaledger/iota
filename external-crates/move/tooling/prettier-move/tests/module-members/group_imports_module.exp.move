// options:
// printWidth: 40
// useModuleLabel: true
// autoGroupImports: module

module prettier::group_imports;

use a::b::{
    Self as c,
    e as f,
    g as f,
    h as i
};
use std::ascii::String as ASCII;
use std::option::{Self as opt, Option};
use std::string::String;
use std::type_name::get as type_name_get;
use std::vector as vec;
use iota::balance::{Self, Balance};
use iota::coin::{Self, Coin};
use iota::dynamic_field as df;
use iota::dynamic_object_field as dof;
use iota::event;
use iota::iota::IOTA;
use iota::transfer_policy::{
    Self,
    TransferPolicy,
    TransferRequest
};

public fun do_something() {}
