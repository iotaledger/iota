// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as Yup from 'yup';
import { createReceivingAddressInputSchema } from './createReceivingAddressInputSchema';
import { createTokenValidation } from './createTokenValidation';
import { SendTokenFormValues } from './formTypes';

export function createValidationSchemaSendTokenForm(
    ...args: Parameters<typeof createTokenValidation>
) {
    return Yup.object({
        ...createReceivingAddressInputSchema(),
        amount: createTokenValidation(...args),
    } satisfies Record<keyof SendTokenFormValues, Yup.AnySchema>);
}
