// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getAmount } from '_helpers';
import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';

export function useGetTransferAmount({
    txn,
    activeAddress,
}: {
    txn: IotaTransactionBlockResponse;
    activeAddress: string;
}) {
    const { effects, events } = txn;
    // const { coins } = getEventsSummary(events!, activeAddress);

    const iotaTransfer = useMemo(() => {
        const txdetails = txn.transaction?.data.transaction;
        return !txdetails
            ? []
            : getAmount(txdetails, effects!, events!)?.map(
                  ({ amount, coinType, recipientAddress }) => {
                      return {
                          amount: amount || 0,
                          coinType: coinType || IOTA_TYPE_ARG,
                          receiverAddress: recipientAddress,
                      };
                  },
              );
    }, [txn, effects, events]);

    // MUSTFIX(chris)
    // const transferAmount = useMemo(() => {
    //     return iotaTransfer?.length
    //         ? iotaTransfer
    //         : coins.filter(
    //               ({ receiverAddress }) => receiverAddress === activeAddress
    //           );
    // }, [iotaTransfer, coins, activeAddress]);

    // return iotaTransfer ?? transferAmount;
    return iotaTransfer;
}
