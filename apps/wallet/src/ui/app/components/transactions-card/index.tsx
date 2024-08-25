// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DateCard } from '_app/shared/date-card';
import { Text } from '_app/shared/text';
import { useGetTxnRecipientAddress } from '_hooks';
import { useRecognizedPackages } from '_src/ui/app/hooks/useRecognizedPackages';
import { getLabel, useTransactionSummary } from '@iota/core';
import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Link } from 'react-router-dom';

import { TxnTypeLabel } from './TxnActionLabel';
import { TxnIcon } from './TxnIcon';

interface TransactionCardProps {
    txn: IotaTransactionBlockResponse;
    address: string;
}

export function TransactionCard({ txn, address }: TransactionCardProps) {
    const executionStatus = txn.effects?.status.status;
    const recognizedPackagesList = useRecognizedPackages();

    const summary = useTransactionSummary({
        transaction: txn,
        currentAddress: address,
        recognizedPackagesList,
    });

    // we only show IOTA Transfer amount or the first non-IOTA transfer amount

    const recipientAddress = useGetTxnRecipientAddress({ txn, address });

    const isSender = address === txn.transaction?.data.sender;

    const error = txn.effects?.status.error;

    // Transition label - depending on the transaction type and amount
    // Epoch change without amount is delegation object
    // Special case for staking and unstaking move call transaction,
    // For other transaction show Sent or Received

    // TODO: Support programmable tx:
    // Show iota symbol only if transfer transferAmount coinType is IOTA_TYPE_ARG, staking or unstaking
    const SHOW_IOTA_SYMBOL = false;

    const timestamp = txn.timestampMs;

    return (
        <Link
            data-testid="link-to-txn"
            to={`/receipt?${new URLSearchParams({
                txdigest: txn.digest,
            }).toString()}`}
            className="flex w-full flex-col items-center gap-2 py-4 no-underline"
        >
            <div className="flex w-full items-start justify-between gap-3">
                <div className="w-7.5">
                    <TxnIcon
                        txnFailed={executionStatus !== 'success' || !!error}
                        // TODO: Support programmable transactions variable icons here:
                        variant={getLabel(txn, address)}
                    />
                </div>
                <div className="flex w-full flex-col gap-1.5">
                    {error ? (
                        <div className="flex w-full justify-between">
                            <div className="flex w-full flex-col gap-1.5">
                                <Text color="gray-90" weight="medium">
                                    Transaction Failed
                                </Text>

                                <div className="flex break-all">
                                    <Text variant="pSubtitle" weight="normal" color="issue-dark">
                                        {error}
                                    </Text>
                                </div>
                            </div>
                            {/* {transferAmountComponent} */}
                        </div>
                    ) : (
                        <>
                            <div className="flex w-full justify-between">
                                <div className="flex items-baseline gap-1 align-middle">
                                    <Text color="gray-90" weight="semibold">
                                        {summary?.label}
                                    </Text>
                                    {SHOW_IOTA_SYMBOL && (
                                        <Text
                                            color="gray-90"
                                            weight="normal"
                                            variant="subtitleSmall"
                                        >
                                            IOTA
                                        </Text>
                                    )}
                                </div>
                                {/* {transferAmountComponent} */}
                            </div>

                            {/* TODO: Support programmable tx: */}
                            <TxnTypeLabel
                                address={recipientAddress!}
                                isSender={isSender}
                                isTransfer={false}
                            />
                            {/* {objectId && <TxnImage id={objectId} />} */}
                        </>
                    )}

                    {timestamp && <DateCard timestamp={Number(timestamp)} size="sm" />}
                </div>
            </div>
        </Link>
    );
}
