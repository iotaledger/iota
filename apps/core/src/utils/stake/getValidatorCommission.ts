// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { formatPercentageDisplay } from '../formatPercentageDisplay';

export function getValidatorCommission(
    validatorData?: IotaValidatorSummary | null,
    hasEffectiveCommissionRate?: boolean,
) {
    const showEffectiveCommissionRate = !!hasEffectiveCommissionRate;
    const commission = validatorData
        ? showEffectiveCommissionRate
            ? Math.max(Number(validatorData.commissionRate), Number(validatorData.votingPower)) /
              100
            : Number(validatorData.commissionRate) / 100
        : 0;
    return formatPercentageDisplay(commission, '--');
}
