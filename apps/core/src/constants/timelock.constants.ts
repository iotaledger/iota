// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const TIMELOCK_MODULE = 'timelock';
export const TIMELOCKED_STAKING_MODULE = 'timelocked_staking';
export const TIMELOCK_IOTA_TYPE = `0x2::${TIMELOCK_MODULE}::TimeLock<0x2::balance::Balance<0x2::iota::IOTA>>`;
export const TIMELOCK_STAKED_TYPE = `0x3::${TIMELOCKED_STAKING_MODULE}::TimelockedStakedIota`;
