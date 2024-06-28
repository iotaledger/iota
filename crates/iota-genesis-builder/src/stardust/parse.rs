// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and logic to parse a full Stardust snapshot.
use std::io::{BufReader, Read};

use anyhow::Result;
use iota_sdk::types::block::{
    output::Output, payload::milestone::MilestoneOption, protocol::ProtocolParameters,
};
use iota_types::stardust::error::StardustError;
use packable::{unpacker::IoUnpacker, Packable};

use super::types::{output_header::OutputHeader, snapshot::FullSnapshotHeader};

/// Parse a Hornet genesis snapshot using a [`BufReader`] internally.
pub struct HornetGenesisSnapshotParser<R: Read> {
    reader: IoUnpacker<BufReader<R>>,
    /// The full-snapshot header
    pub header: FullSnapshotHeader,
}

impl<R: Read> HornetGenesisSnapshotParser<R> {
    pub fn new(reader: R) -> Result<Self> {
        let mut reader = IoUnpacker::new(std::io::BufReader::new(reader));
        // `true` ensures that only genesis snapshots unpack successfully
        let header = FullSnapshotHeader::unpack::<_, true>(&mut reader, &())?;

        Ok(Self { reader, header })
    }

    /// Provide an iterator over the Stardust UTXOs recorded in the snapshot.
    pub fn outputs(mut self) -> impl Iterator<Item = anyhow::Result<(OutputHeader, Output)>> {
        (0..self.header.output_count()).map(move |_| {
            Ok((
                OutputHeader::unpack::<_, true>(&mut self.reader, &())?,
                Output::unpack::<_, true>(&mut self.reader, &ProtocolParameters::default())?,
            ))
        })
    }

    /// Provide the target milestone timestamp extracted from the snapshot
    /// header.
    pub fn target_milestone_timestamp(&self) -> u32 {
        self.header.target_milestone_timestamp()
    }

    /// Provide the network main token total supply through the snapshot
    /// protocol parameters.
    pub fn total_supply(&self) -> Result<u64> {
        if let MilestoneOption::Parameters(params) = self.header.parameters_milestone_option() {
            let protocol_params = <ProtocolParameters as packable::PackableExt>::unpack_unverified(
                params.binary_parameters(),
            )
            .expect("invalid protocol params");
            Ok(protocol_params.token_supply())
        } else {
            Err(StardustError::HornetSnapshotParametersNotFound.into())
        }
    }
}
