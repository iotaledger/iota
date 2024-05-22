// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload, type BasePayload } from '../BasePayload';
import { type Payload } from '../Payload';

export interface DeriveAddressResponse extends BasePayload {
	type: 'derive-address-response';
	address: string
}

export function isDeriveAddressResponse(payload: Payload): payload is DeriveAddressResponse {
	return isBasePayload(payload) && payload.type === 'derive-address-response';
}
