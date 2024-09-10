// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getAmount } from '_helpers';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useMemo } from 'react';

type Props = {
    txn: IotaTransactionBlockResponse;
    address: string;
};

export function useGetTxnRecipientAddress({ txn, address }: Props) {
    const events = txn.events!;

    // const eventsSummary = useMemo(() => {
    //     const { coins } = getEventsSummary(events, address);
    //     return coins;
    // }, [events, address]);

    const transaction = txn.transaction?.data.transaction;
    const amountByRecipient = !transaction ? null : getAmount(transaction, txn.effects!, events);

    const recipientAddress = useMemo(() => {
        const transferObjectRecipientAddress =
            amountByRecipient &&
            amountByRecipient?.find(({ recipientAddress }) => recipientAddress !== address)
                ?.recipientAddress;
        // MUSTFIX(chris)
        // const receiverAddr =
        //     eventsSummary &&
        //     eventsSummary.find(
        //         ({ receiverAddress }) => receiverAddress !== address
        //     )?.receiverAddress;

        return null ?? transferObjectRecipientAddress ?? txn.transaction?.data.sender;
    }, [address, amountByRecipient, txn]);
    // }, [address, amountByRecipient, eventsSummary, txn]);

    return recipientAddress;
}
