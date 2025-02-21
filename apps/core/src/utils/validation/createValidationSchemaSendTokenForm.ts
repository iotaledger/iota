// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Yup from 'yup';
import { createIotaAddressValidation } from './createIotaAddressValidation';
import { createTokenValidation } from './createTokenValidation';

export function createValidationSchemaSendTokenForm(
    ...args: Parameters<typeof createTokenValidation>
) {
    return Yup.object({
        to: createIotaAddressValidation(),
        amount: createTokenValidation(...args),
    });
}
