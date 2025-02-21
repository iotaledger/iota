// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and logic to parse a full Stardust snapshot.
use std::io::{BufReader, Read};

use anyhow::Result;
use iota_sdk::types::block::{
    output::Output, payload::milestone::MilestoneOption, protocol::ProtocolParameters,
};
use iota_types::stardust::error::StardustError;
use packable::{
    Packable,
    unpacker::{IoUnpacker, Unpacker},
};

use super::types::{output_header::OutputHeader, snapshot::FullSnapshotHeader};

/// Parse a Hornet genesis snapshot using a [`BufReader`] internally.
pub struct HornetSnapshotParser<R: Read> {
    reader: IoUnpacker<BufReader<R>>,
    /// The full-snapshot header
    pub header: FullSnapshotHeader,
}

impl<R: Read> HornetSnapshotParser<R> {
    /// Creates a new [`HornetSnapshotParser`].
    ///
    /// `VERIFY = true` ensures that only global snapshots parse successfully.
    pub fn new<const VERIFY: bool>(reader: R) -> Result<Self> {
        let mut reader = IoUnpacker::new(std::io::BufReader::new(reader));
        let header = FullSnapshotHeader::unpack::<_, VERIFY>(&mut reader, &())?;
        Ok(Self { reader, header })
    }

    /// Provide an iterator over the Stardust UTXOs recorded in the snapshot.
    pub fn outputs(&mut self) -> impl Iterator<Item = anyhow::Result<(OutputHeader, Output)>> + '_ {
        let protocol_params = self.protocol_parameters().unwrap_or_default();

        (0..self.header.output_count()).map(move |_| {
            Ok((
                OutputHeader::unpack::<_, true>(&mut self.reader, &())?,
                Output::unpack::<_, true>(&mut self.reader, &protocol_params)?,
            ))
        })
    }

    /// Get the bytes of the solid entry points.
    pub fn solid_entry_points_bytes(mut self) -> anyhow::Result<Vec<u8>> {
        let mut remaining_bytes = vec![];
        // Workaround as .read_to_end() is not available
        let mut next_byte = vec![0u8; 1];
        while self.reader.unpack_bytes(&mut next_byte).is_ok() {
            remaining_bytes.push(next_byte[0]);
        }

        let sep_bytes = remaining_bytes
            .get(remaining_bytes.len() - self.header.sep_count() as usize * 32..)
            .expect("missing SEP bytes")
            .to_vec();

        Ok(sep_bytes)
    }

    /// Provide the target milestone timestamp extracted from the snapshot
    /// header.
    pub fn target_milestone_timestamp(&self) -> u32 {
        self.header.target_milestone_timestamp()
    }

    /// Provide the network main token total supply through the snapshot
    /// protocol parameters.
    pub fn total_supply(&self) -> Result<u64> {
        Ok(self.protocol_parameters()?.token_supply())
    }

    /// Get the protocol parameters.
    pub fn protocol_parameters(&self) -> Result<ProtocolParameters> {
        if let MilestoneOption::Parameters(params) = self.header.parameters_milestone_option() {
            Ok(
                <ProtocolParameters as packable::PackableExt>::unpack_unverified(
                    params.binary_parameters(),
                )
                .expect("invalid protocol params"),
            )
        } else {
            Err(StardustError::HornetSnapshotParametersNotFound.into())
        }
    }
}
