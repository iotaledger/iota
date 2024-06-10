// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and logic to parse a full Stardust snapshot.
use std::io::{BufReader, Read};

use anyhow::Result;
use iota_sdk::types::block::{output::Output, protocol::ProtocolParameters};
use packable::{unpacker::IoUnpacker, Packable};

use super::types::snapshot::{FullSnapshotHeader, OutputHeader};

/// Parse a full-snapshot using a [`BufReader`] internally.
pub struct FullSnapshotParser<R: Read> {
    reader: IoUnpacker<BufReader<R>>,
    /// The full-snapshot header
    pub header: FullSnapshotHeader,
}

impl<R: Read> FullSnapshotParser<R> {
    pub fn new(reader: R) -> Result<Self> {
        let mut reader = IoUnpacker::new(std::io::BufReader::new(reader));
        let header = FullSnapshotHeader::unpack::<_, true>(&mut reader, &())?;

        Ok(Self { reader, header })
    }

    /// Provide an iterator over the Stardust UTXOs recorded in the snapshot.
    pub fn outputs(mut self) -> impl Iterator<Item = Result<Output, anyhow::Error>> {
        (0..self.header.output_count()).map(move |_| {
            let _header = OutputHeader::unpack::<_, true>(&mut self.reader, &())?;

            Ok(Output::unpack::<_, true>(
                &mut self.reader,
                &ProtocolParameters::default(),
            )?)
        })
    }
}
