// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Transaction, TransactionObjectInput } from './Transaction.js';
import { Inputs } from './Inputs.js';

export function createObjectMethods<T>(makeObject: (value: TransactionObjectInput) => T) {
    function object(value: TransactionObjectInput) {
        return makeObject(value);
    }

    object.system = (options?: { mutable?: boolean }) => {
        const mutable = options?.mutable;

        if (mutable !== undefined) {
            return object(
                Inputs.SharedObjectRef({
                    objectId: '0x5',
                    initialSharedVersion: 1,
                    mutable,
                }),
            );
        }

        return object({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x5',
                initialSharedVersion: 1,
            },
        });
    };
    object.clock = () =>
        object(
            Inputs.SharedObjectRef({
                objectId: '0x6',
                initialSharedVersion: 1,
                mutable: false,
            }),
        );
    object.random = () =>
        object({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x8',

                mutable: false,
            },
        });
    object.denyList = (options?: { mutable?: boolean }) => {
        return object({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x403',
                mutable: options?.mutable,
            },
        });
    };
    object.option =
        ({ type, value }: { type: string; value: TransactionObjectInput | null }) =>
        (tx: Transaction) =>
            tx.moveCall({
                typeArguments: [type],
                target: `0x1::option::${value === null ? 'none' : 'some'}`,
                arguments: value === null ? [] : [tx.object(value)],
            });

    return object;
}
