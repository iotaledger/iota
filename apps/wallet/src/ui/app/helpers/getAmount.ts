// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent, IotaTransactionBlockKind, TransactionEffects } from '@mysten/iota.js/client';

type FormattedBalance = {
	amount?: number | null;
	coinType?: string | null;
	recipientAddress: string;
}[];

export function getAmount(
	_txnData: IotaTransactionBlockKind,
	_txnEffect: TransactionEffects,
	_events: IotaEvent[],
): FormattedBalance | null {
	// TODO: Support programmable transactions:
	return null;
}
