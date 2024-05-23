// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload, type BasePayload } from '../BasePayload';
import { type Payload } from '../Payload';

export interface DeriveAddressRequest extends BasePayload {
	type: 'derive-address-request';
	accountIndex: number,
	addressIndex: number
}

export function isDeriveAddressRequest(payload: Payload): payload is DeriveAddressRequest {
	return isBasePayload(payload) && payload.type === 'derive-address-request';
}
