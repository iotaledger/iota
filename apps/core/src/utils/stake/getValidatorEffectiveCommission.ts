// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { formatPercentageDisplay } from '../formatPercentageDisplay';

export function getValidatorEffectiveCommission(validatorData?: IotaValidatorSummary | null) {
    const effectiveCommission = validatorData
        ? Number(validatorData.effectiveCommissionRate) / 100
        : 0;
    return formatPercentageDisplay(effectiveCommission, '--');
}
