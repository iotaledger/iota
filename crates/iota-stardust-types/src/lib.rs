// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(clippy::nursery, rust_2018_idioms, warnings, unreachable_pub)]
#![allow(
    clippy::redundant_pub_crate,
    clippy::missing_const_for_fn,
    clippy::significant_drop_in_scrutinee,
    clippy::significant_drop_tightening,
    clippy::empty_docs
)]

extern crate alloc;

pub mod types;
