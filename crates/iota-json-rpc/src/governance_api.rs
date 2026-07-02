// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::ValidatorApy;
use iota_sdk_types::{Address, ObjectId};
use iota_types::{committee::EpochId, iota_system_state::PoolTokenExchangeRate};
use itertools::Itertools;
use statrs::statistics::{Data, Median};

pub fn calculate_apys(exchange_rate_table: Vec<ValidatorExchangeRates>) -> Vec<ValidatorApy> {
    let mut apys = vec![];

    for rates in exchange_rate_table.into_iter().filter(|r| r.active) {
        let exchange_rates = rates.rates.iter().map(|(_, rate)| rate);

        let mean_apy = mean_apy_from_exchange_rates(exchange_rates);
        apys.push(ValidatorApy {
            address: rates.address,
            apy: mean_apy,
        });
    }
    apys
}

/// Calculate the APY using a 7-epoch moving average.
///
/// Returns the Mean by default, but falls back to the Median if outliers are
/// detected. Outliers are defined as any APY > `MAX_VALID_APY` (100%) or if the
/// trailing 8th epoch exchange rate is missing. This fallback protects against
/// skewed results caused by large staking events or the spikes seen after
/// missing exchange rates.
pub fn mean_apy_from_exchange_rates<'er>(
    exchange_rates: impl DoubleEndedIterator<Item = &'er PoolTokenExchangeRate> + Clone,
) -> f64 {
    // We set this value after observing the APY of validators in mainnet.
    const MAX_VALID_APY: f64 = 1.00;
    const SAMPLES: usize = 7;

    let rates = exchange_rates.clone().dropping(1);
    let rates_next = exchange_rates.dropping_back(1);

    let mut apys = rates
        .zip(rates_next)
        .take(SAMPLES + 1)
        .map(|(er, er_next)| calculate_apy(er, er_next))
        .collect::<Vec<_>>();

    // Return 0.0 if there is no data OR if any APY is negative
    if apys.is_empty() || apys.iter().any(|&apy| apy < 0.0) {
        return 0.0;
    }
    // If any single epoch has outliers (that is APY > MAX_VALID_APY or exchange
    // rate for epoch e-8 is missing), we switch to Median. Otherwise, we use
    // the standard Mean.
    let has_outlier = apys.get(SAMPLES).is_some_and(|&apy| apy <= 0.0)
        || apys.iter().any(|&apy| apy > MAX_VALID_APY);

    apys.truncate(SAMPLES);

    if has_outlier {
        Data::new(apys).median()
    } else {
        let sum: f64 = apys.iter().sum();
        sum / SAMPLES as f64
    }
}

/// APY magnitudes below this threshold are treated as exactly zero.
const APY_DUST_THRESHOLD: f64 = 1e-9;

/// Calculate the APY from the exchange rate of two consecutive epochs
/// (`er` is the older epoch, `er_next` the newer one).
///
/// The formula used is `APY_e = (er.rate - er_next.rate) / er_next.rate * 365`.
fn calculate_apy(er: &PoolTokenExchangeRate, er_next: &PoolTokenExchangeRate) -> f64 {
    let apy = ((er.rate() - er_next.rate()) / er_next.rate()) * 365.0;
    if apy.abs() < APY_DUST_THRESHOLD {
        0.0
    } else {
        apy
    }
}

#[derive(Clone, Debug)]
pub struct ValidatorExchangeRates {
    pub address: Address,
    pub pool_id: ObjectId,
    pub active: bool,
    pub rates: Vec<(EpochId, PoolTokenExchangeRate)>,
}

/// Backfill missing rates for some epochs due to safe mode. If a rate is
/// missing for epoch e, we will use the rate for epoch e-1 to fill it. Rates
/// returned are in descending order by epoch.
/// Backfill missing rates for epochs skipped due to safe mode: a rate missing
/// for epoch `e` is filled from epoch `e - 1`. Returns rates in descending
/// epoch order.
pub fn backfill_rates(
    mut rates: Vec<(EpochId, PoolTokenExchangeRate)>,
) -> Vec<(EpochId, PoolTokenExchangeRate)> {
    if rates.is_empty() {
        return rates;
    }
    // ensure epochs are processed in increasing order
    rates.sort_unstable_by_key(|(epoch_id, _)| *epoch_id);

    // Check if there are any gaps in the epochs
    let (min_epoch, _) = rates.first().expect("rates should not be empty");
    let (max_epoch, _) = rates.last().expect("rates should not be empty");
    let expected_len = (max_epoch - min_epoch + 1) as usize;
    let current_len = rates.len();

    // Only perform backfilling if there are gaps
    if current_len == expected_len {
        rates.reverse();
        return rates;
    }

    let mut filled_rates: Vec<(EpochId, PoolTokenExchangeRate)> = Vec::with_capacity(expected_len);
    let mut missing_rates = Vec::with_capacity(expected_len - current_len);
    for (epoch_id, rate) in rates {
        // fill gaps between the last processed epoch and the current one
        if let Some((prev_epoch_id, prev_rate)) = filled_rates.last() {
            for missing_epoch_id in prev_epoch_id + 1..epoch_id {
                missing_rates.push((missing_epoch_id, prev_rate.clone()));
            }
        };

        // append any missing_rates before adding the current epoch.
        // if empty, nothing gets appended.
        // if not empty, it will be empty afterwards because it was moved into
        // filled_rates
        filled_rates.append(&mut missing_rates);
        filled_rates.push((epoch_id, rate));
    }
    filled_rates.reverse();
    filled_rates
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iota_types::iota_system_state::PoolTokenExchangeRate;

    use super::*;

    #[test]
    fn calculate_apys_with_outliers() {
        let file =
            std::fs::File::open("src/unit_tests/data/validator_exchange_rate/rates-test.json")
                .unwrap();
        let rates: BTreeMap<String, Vec<(u64, PoolTokenExchangeRate)>> =
            serde_json::from_reader(file).unwrap();

        let mut address_map = BTreeMap::new();

        let exchange_rates = rates
            .into_iter()
            .map(|(validator, rates_vec)| {
                let address = Address::random();
                address_map.insert(address, validator);
                ValidatorExchangeRates {
                    address,
                    pool_id: ObjectId::random(),
                    active: true,
                    rates: backfill_rates(rates_vec),
                }
            })
            .collect();

        let apys = calculate_apys(exchange_rates);

        for apy in &apys {
            println!("{}: {}", address_map[&apy.address], apy.apy);
            assert!(apy.apy < 0.15)
        }
    }

    #[test]
    fn calculate_apys_without_outliers() {
        let file =
            std::fs::File::open("src/unit_tests/data/validator_exchange_rate/rates-feb26.json")
                .unwrap();
        let rates: BTreeMap<String, Vec<(u64, PoolTokenExchangeRate)>> =
            serde_json::from_reader(file).unwrap();

        let mut address_map = BTreeMap::new();

        let exchange_rates = rates
            .into_iter()
            .map(|(validator, rates_vec)| {
                let address = Address::random();
                address_map.insert(address, validator);
                ValidatorExchangeRates {
                    address,
                    pool_id: ObjectId::random(),
                    active: true,
                    rates: backfill_rates(rates_vec),
                }
            })
            .collect();

        let apys = calculate_apys(exchange_rates);

        for apy in &apys {
            println!("{}: {}", address_map[&apy.address], apy.apy);
            assert!(apy.apy < 0.15)
        }
    }

    #[test]
    fn calculate_apy_is_not_negative_for_zero_reward_epoch() {
        // Real mainnet exchange rates for two validators transitioning from
        // epoch 381 to 382, an epoch in which they earned no rewards. The rate is
        // therefore unchanged up to integer-truncation dust, so `calculate_apy`
        // must report an effectively-zero APY (within
        // `[0, APY_DUST_THRESHOLD)`).
        let cases = [
            (
                (48_913_429_030_426_080u64, 43_331_127_650_932_384u64),
                (48_641_042_011_532_656u64, 43_089_827_114_043_304u64),
            ),
            (
                (33_370_417_056_337_732u64, 29_578_114_234_284_444u64),
                (33_370_374_157_145_896u64, 29_578_076_210_270_704u64),
            ),
        ];

        for ((i_old, t_old), (i_new, t_new)) in cases {
            let er = PoolTokenExchangeRate::new_for_testing(i_old, t_old);
            let er_next = PoolTokenExchangeRate::new_for_testing(i_new, t_new);
            let apy = calculate_apy(&er, &er_next);
            assert!(
                (0.0..APY_DUST_THRESHOLD).contains(&apy),
                "expected an effectively-zero, non-negative APY, got {apy}"
            );
        }
    }

    #[test]
    fn test_backfill_rates_empty() {
        let rates = vec![];
        assert_eq!(backfill_rates(rates), vec![]);
    }

    #[test]
    fn test_backfill_rates_no_gaps() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate2 = PoolTokenExchangeRate::new_for_testing(200, 220);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rates = vec![(2, rate2.clone()), (3, rate3.clone()), (1, rate1.clone())];

        let expected: Vec<(u64, PoolTokenExchangeRate)> = vec![(3, rate3), (2, rate2), (1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_single_rate() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rates = vec![(1, rate1.clone())];
        let expected = vec![(1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_with_gaps() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rate5 = PoolTokenExchangeRate::new_for_testing(500, 550);
        let rates = vec![(3, rate3.clone()), (1, rate1.clone()), (5, rate5.clone())];

        let expected = vec![
            (5, rate5),
            (4, rate3.clone()),
            (3, rate3),
            (2, rate1.clone()),
            (1, rate1),
        ];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_missing_middle_epoch() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rates = vec![(1, rate1.clone()), (3, rate3.clone())];
        let expected = vec![(3, rate3), (2, rate1.clone()), (1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_missing_middle_epochs() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate4 = PoolTokenExchangeRate::new_for_testing(400, 440);
        let rates = vec![(1, rate1.clone()), (4, rate4.clone())];
        let expected = vec![
            (4, rate4),
            (3, rate1.clone()),
            (2, rate1.clone()),
            (1, rate1),
        ];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_unordered_input() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rate4 = PoolTokenExchangeRate::new_for_testing(400, 440);
        let rates = vec![(3, rate3.clone()), (1, rate1.clone()), (4, rate4.clone())];
        let expected = vec![(4, rate4), (3, rate3), (2, rate1.clone()), (1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }
}
