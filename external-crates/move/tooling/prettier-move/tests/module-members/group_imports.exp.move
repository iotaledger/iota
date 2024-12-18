// options:
// printWidth: 40
// useModuleLabel: true
// autoGroupImports: package

module prettier::group_imports;

use 0x0::{
    Account::{Self, Account},
    Something
};
use std::{
    ascii::String as ASCII,
    option::{Self as opt, Option},
    string::String as UTF8,
    type_name::get as type_name_get,
    vector::{Self as vec, Self as haha}
};
use iota::{
    clock::Clock,
    coin::Coin,
    dynamic_field as df,
    dynamic_object_field as dof,
    iota::IOTA,
    table::{Self, Table},
    table_vec::{Self, TableVec as TV}
};
