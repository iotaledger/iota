// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { formatPercentageDisplay } from '../formatPercentageDisplay';

export function getValidatorCommission(validatorData?: IotaValidatorSummary | null) {
    const commission = validatorData ? Number(validatorData.commissionRate) / 100 : 0;
    return formatPercentageDisplay(commission, '--');
}
