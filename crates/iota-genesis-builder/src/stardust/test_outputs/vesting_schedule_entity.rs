// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Entity vesting schedule scenario.
//! 4-years, initial unlock, bi-weekly unlock.
//! One mnemonic, one account, one address.

use iota_stardust_types::block::output::Output;
use rand::{Rng, rngs::StdRng};

use crate::stardust::{
    test_outputs::{MERGE_TIMESTAMP_SECS, address_derivation, new_vested_output},
    types::output_header::OutputHeader,
};

const MNEMONIC: &str = "chunk beach oval twist manage spread street width view pig hen oak size fix lab tent say home team cube loop van they suit";

const VESTING_WEEKS: usize = 208;
const VESTING_WEEKS_FREQUENCY: usize = 2;

pub(crate) fn outputs(
    rng: &mut StdRng,
    vested_index: &mut u32,
    coin_type: u32,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = Vec::new();
    let address = address_derivation::derive_address(MNEMONIC, coin_type, 0, 0, false)?;
    // VESTING_WEEKS / VESTING_WEEKS_FREQUENCY * 10 so that `vested_amount` doesn't
    // lose precision.
    let amount = rng.gen_range(1_000_000..10_000_000)
        * (VESTING_WEEKS as u64 / VESTING_WEEKS_FREQUENCY as u64 * 10);
    // Initial unlock amount is 10% of the total address reward.
    let initial_unlock_amount = amount * 10 / 100;
    // Vested amount is 90% of the total address reward spread across the vesting
    // schedule.
    let vested_amount = amount * 90 / 100 / (VESTING_WEEKS as u64 / VESTING_WEEKS_FREQUENCY as u64);

    outputs.push(new_vested_output(
        *vested_index,
        initial_unlock_amount,
        address,
        None,
        rng,
    )?);
    *vested_index -= 1;

    for offset in (0..=VESTING_WEEKS).step_by(VESTING_WEEKS_FREQUENCY) {
        let timelock = MERGE_TIMESTAMP_SECS + offset as u32 * 604_800;

        outputs.push(new_vested_output(
            *vested_index,
            vested_amount,
            address,
            Some(timelock),
            rng,
        )?);
        *vested_index -= 1;
    }

    Ok(outputs)
}
