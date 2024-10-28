// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod coin_read;
mod event;
mod governance;
mod quorum_driver;
mod read;

pub use self::{
    coin_read::CoinReadApi, event::EventApi, governance::GovernanceApi,
    quorum_driver::QuorumDriverApi, read::ReadApi,
};

pub enum Order {
    Ascending,
    Descending,
}

impl Order {
    pub fn is_ascending(&self) -> bool {
        match self {
            Order::Ascending => true,
            Order::Descending => false,
        }
    }

    pub fn is_descending(&self) -> bool {
        !self.is_ascending()
    }
}
