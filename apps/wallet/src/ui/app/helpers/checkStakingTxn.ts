// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SuiTransactionBlockResponse } from '@mysten/sui.js/client';

// TODO: Support programmable transactions:
export function checkStakingTxn(_txn: SuiTransactionBlockResponse) {
	return false;
}
