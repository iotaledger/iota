// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use iota_sdk_types::ObjectId;
use iota_types::base_types::IotaAddress;
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Board {
    pub id: ObjectId,
    pub marks: Vec<u8>,
    pub turn: u8,
    pub x: IotaAddress,
    pub o: IotaAddress,
}

#[derive(Eq, PartialEq)]
pub(crate) enum Player {
    X,
    O,
}

impl Board {
    pub(crate) fn next_player(&self) -> Player {
        if self.turn.is_multiple_of(2) {
            Player::X
        } else {
            Player::O
        }
    }

    pub(crate) fn prev_player(&self) -> Player {
        if self.turn.is_multiple_of(2) {
            Player::O
        } else {
            Player::X
        }
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let m = |i: usize| match self.marks[i] {
            0 => ' ',
            1 => 'X',
            2 => 'O',
            _ => unreachable!(),
        };

        writeln!(f, "{: >31} {} | {} | {}", ' ', m(0), m(1), m(2))?;
        writeln!(f, "{: >31}---+---+---", ' ')?;
        writeln!(f, "{: >31} {} | {} | {}", ' ', m(3), m(4), m(5))?;
        writeln!(f, "{: >31}---+---+---", ' ')?;
        writeln!(f, "{: >31} {} | {} | {}", ' ', m(6), m(7), m(8))?;
        writeln!(f)?;

        use Player as P;
        let next = self.next_player();

        write!(f, "{}", if next == P::X { " -> " } else { "    " })?;
        writeln!(f, "X: {}", self.x)?;

        write!(f, "{}", if next == P::O { " -> " } else { "    " })?;
        writeln!(f, "O: {}", self.o)?;

        write!(f, " GAME: {}", self.id)?;

        Ok(())
    }
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Player::X => write!(f, "X"),
            Player::O => write!(f, "O"),
        }
    }
}
