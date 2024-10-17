// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isValidIotaAddress } from '@iota/iota-sdk/utils';
import * as Yup from 'yup';
import { ValidationError } from 'yup';

export { ValidationError };

export function createIotaAddressValidation() {
    return Yup.string()
        .ensure()
        .trim()
        .required()
        .test('is-iota-address', 'Invalid address. Please check again.', async (value) => {
            return isValidIotaAddress(value);
        })
        .label("Recipient's address");
}
