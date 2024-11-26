// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaValidatorSummary } from '@iota/iota-sdk/client';

export function getTotalValidatorStake(validatorSummary: IotaValidatorSummary | null) {
    return validatorSummary?.stakingPoolIotaBalance || 0;
}
