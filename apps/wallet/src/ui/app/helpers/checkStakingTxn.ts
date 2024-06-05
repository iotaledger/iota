// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaTransactionBlockResponse } from '@iota/iota.js/client';

// TODO: Support programmable transactions:
export function checkStakingTxn(_txn: IotaTransactionBlockResponse) {
    return false;
}
