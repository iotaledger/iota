// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::{Read, Write},
    net::TcpStream,
};

use ledger_transport::{APDUAnswer, APDUCommand};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerTCPError {
    #[error("Ledger connect error")]
    ConnectFailed,
    #[error("Invalid TCP response error")]
    InvalidResponse,
    #[error("Ledger inner error")]
    Inner,
}

pub struct TransportTCP {
    url: String,
}

impl TransportTCP {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            url: format!("{host}:{port}"),
        }
    }

    fn request(raw_command: &[u8], stream: &mut TcpStream) -> Result<Vec<u8>, std::io::Error> {
        // store length as 32bit big endian into array
        let send_length_bytes = (raw_command.len() as u32).to_be_bytes();

        // first send number of bytes
        stream.write_all(&send_length_bytes[..])?;

        // then send bytes
        stream.write_all(raw_command)?;

        let mut rcv_length_bytes = [0u8; 4];

        // first read number of bytes
        stream.read_exact(&mut rcv_length_bytes)?;

        // convert bytes from big endian (+2 for return code)
        let rcv_length = u32::from_be_bytes(rcv_length_bytes) + 2;

        let mut buf = vec![0u8; rcv_length as usize];
        stream.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn exchange(
        &self,
        command: &APDUCommand<Vec<u8>>,
    ) -> Result<APDUAnswer<Vec<u8>>, LedgerTCPError> {
        let raw_command = command.serialize();

        let mut stream =
            TcpStream::connect(&self.url).map_err(|_| LedgerTCPError::ConnectFailed)?;

        let raw_answer =
            TransportTCP::request(&raw_command, &mut stream).map_err(|_| LedgerTCPError::Inner)?;
        let answer =
            APDUAnswer::from_answer(raw_answer).map_err(|_| LedgerTCPError::InvalidResponse)?;

        Ok(answer)
    }
}
