// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Get a random number generator.
#[inline(always)]
pub fn get_rng() -> impl rand::Rng {
    rand::thread_rng()
}

// Need our own Either implementation so we can impl rand::RngCore for it.
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> rand::RngCore for Either<L, R>
where
    L: rand::RngCore,
    R: rand::RngCore,
{
    fn next_u32(&mut self) -> u32 {
        match self {
            Either::Left(l) => l.next_u32(),
            Either::Right(r) => r.next_u32(),
        }
    }

    fn next_u64(&mut self) -> u64 {
        match self {
            Either::Left(l) => l.next_u64(),
            Either::Right(r) => r.next_u64(),
        }
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        match self {
            Either::Left(l) => l.fill_bytes(dest),
            Either::Right(r) => r.fill_bytes(dest),
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        match self {
            Either::Left(l) => l.try_fill_bytes(dest),
            Either::Right(r) => r.try_fill_bytes(dest),
        }
    }
}
