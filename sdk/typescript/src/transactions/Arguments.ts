// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Argument } from './data/internal.js';
import type { Inputs } from './Inputs.js';
import { createObjectMethods } from './object.js';
import { createPure } from './pure.js';
import type { Transaction, TransactionObjectArgument } from './Transaction.js';

export const Arguments = {
    pure: createPure<(tx: Transaction) => Argument>(
        (value) => (tx) => tx.pure(value),
    ) as ReturnType<typeof createPure<(tx: Transaction) => Argument>>,
    object: createObjectMethods<TransactionObjectArgument>((value) => (tx) => tx.object(value)),
    sharedObjectRef:
        (...args: Parameters<(typeof Inputs)['SharedObjectRef']>) =>
        (tx: Transaction) =>
            tx.sharedObjectRef(...args),
    objectRef:
        (...args: Parameters<(typeof Inputs)['ObjectRef']>) =>
        (tx: Transaction) =>
            tx.objectRef(...args),
    receivingRef:
        (...args: Parameters<(typeof Inputs)['ReceivingRef']>) =>
        (tx: Transaction) =>
            tx.receivingRef(...args),
};
