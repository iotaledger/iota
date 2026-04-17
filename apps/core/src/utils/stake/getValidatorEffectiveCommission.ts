// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { formatPercentageDisplay } from '../formatPercentageDisplay';

export function getValidatorEffectiveCommission(validatorData?: IotaValidatorSummary | null) {
    const rate = validatorData?.effectiveCommissionRate;
    const effectiveCommission = rate != null ? Number(rate) / 100 : 0;
    return formatPercentageDisplay(effectiveCommission, '--');
}
