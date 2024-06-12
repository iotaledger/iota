// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState, useCallback } from 'react';
import { TransactionBlock } from '@iota/iota.js/transactions';
import { IOTA_SYSTEM_STATE_OBJECT_ID } from '@iota/iota.js/utils';

interface NewStake {
    transaction: TransactionBlock | null;
    createTransaction: (amount: bigint, validator: string) => void;
    loading: boolean;
    error: unknown;
}

function createStakeTransaction(amount: bigint, validator: string) {
    const tx = new TransactionBlock();
    const stakeCoin = tx.splitCoins(tx.gas, [amount]);
    tx.moveCall({
        target: '0x3::iota_system::request_add_stake',
        arguments: [
            tx.sharedObjectRef({
                objectId: IOTA_SYSTEM_STATE_OBJECT_ID,
                initialSharedVersion: 1,
                mutable: true,
            }),
            stakeCoin,
            tx.pure.address(validator),
        ],
    });
    return tx;
}

export function useNewStake(): NewStake {
    const [transaction, setTransaction] = useState<TransactionBlock | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<unknown>(null);

    const createTransaction = useCallback((amount: bigint, validator: string) => {
        setLoading(true);
        setError(null);

        try {
            const tx = createStakeTransaction(amount, validator);
            setTransaction(tx);
        } catch (err) {
            setError(err);
        } finally {
            setLoading(false);
        }
    }, []);

    return { transaction, createTransaction, loading, error };
}
