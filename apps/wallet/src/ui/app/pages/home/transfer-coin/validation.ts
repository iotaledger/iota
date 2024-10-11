// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createIotaAddressValidation } from '@iota/core';
import { createTokenValidation } from '_src/shared/validation';
import * as Yup from 'yup';

export function createValidationSchemaStepOne(...args: Parameters<typeof createTokenValidation>) {
    return Yup.object({
        to: createIotaAddressValidation(),
        amount: createTokenValidation(...args),
    });
}
