// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod vesting_schedule_multiple_addresses;

use std::{
    fs::{File, OpenOptions},
    io::BufWriter,
    path::Path,
};

use packable::{packer::IoPacker, Packable};

use crate::stardust::parse::FullSnapshotParser;

pub fn add_snapshot_test_data<P: AsRef<Path> + core::fmt::Debug>(
    current_path: P,
    new_path: P,
) -> anyhow::Result<()> {
    let current_file = File::open(current_path)?;
    let new_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(new_path)?;
    let mut writer = IoPacker::new(BufWriter::new(new_file));

    let mut parser = FullSnapshotParser::new(current_file)?;

    let new_outputs = vesting_schedule_multiple_addresses::outputs();

    parser.header.output_count += new_outputs.len() as u64;

    parser.header.pack(&mut writer)?;

    parser.outputs().for_each(|res| {
        let (output_header, output) = res.unwrap();
        output_header.pack(&mut writer).unwrap();
        output.pack(&mut writer).unwrap();
    });

    Ok(())
}
