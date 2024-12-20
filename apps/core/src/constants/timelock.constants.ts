// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { COIN_TYPE } from "./coins.constants";

export const TIMELOCK_IOTA_TYPE = `0x2::timelock::TimeLock<0x2::balance::Balance<${COIN_TYPE}>>`;
export const TIMELOCK_STAKED_TYPE = '0x3::timelocked_staking::TimelockedStakedIota';
