// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createIotaAddressValidation } from '_components/address-input/validation';
import { createTokenValidation } from '_src/shared/validation';
import { type IotaClient } from '@iota/iota-sdk/client';
import * as Yup from 'yup';

export function createValidationSchemaStepOne(
	client: IotaClient,
	iotaNSEnabled: boolean,
	...args: Parameters<typeof createTokenValidation>
) {
	return Yup.object({
		to: createIotaAddressValidation(client, iotaNSEnabled),
		amount: createTokenValidation(...args),
	});
}
