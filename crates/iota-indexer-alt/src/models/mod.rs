// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel_migrations::{EmbeddedMigrations, embed_migrations};

pub mod checkpoints;
pub mod displays;
pub mod epochs;
pub mod events;
pub mod objects;
pub mod packages;
pub mod transactions;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
