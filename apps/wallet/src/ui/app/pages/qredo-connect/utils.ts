// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type UIQredoPendingRequest } from '_src/background/qredo/types';

export function isUntrustedQredoConnect({ apiUrl, origin }: UIQredoPendingRequest) {
	try {
		return new URL(origin).protocol !== 'https:' || new URL(apiUrl).protocol !== 'https:';
	} catch (e) {
		return false;
	}
}
