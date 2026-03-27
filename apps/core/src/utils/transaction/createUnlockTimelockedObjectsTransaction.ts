// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createCollectAllTimelocksTransaction } from './createCollectAllTimelocksTransaction';

interface CreateUnlockTimelockedObjectTransactionOptions {
    address: string;
    objectIds: string[];
}

export function createUnlockTimelockedObjectsTransaction({
    address,
    objectIds,
}: CreateUnlockTimelockedObjectTransactionOptions) {
    return createCollectAllTimelocksTransaction({
        address,
        timelockObjectIds: objectIds,
    });
}
