// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useTransactionSummary,
    TransactionReceipt,
    ExplorerLinkType,
    ViewTxnOnExplorerButton,
    useRecognizedPackages,
    toast,
    OutlinedCopyButton,
} from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { ExplorerLinkHelper } from '../ExplorerLinkHelper';
import { ExplorerLink } from '../explorer-link';

interface ReceiptCardProps {
    txn: IotaTransactionBlockResponse;
    activeAddress: string;
}

export function ReceiptCard({ txn, activeAddress }: ReceiptCardProps) {
    const recognizedPackagesList = useRecognizedPackages();
    const summary = useTransactionSummary({
        transaction: txn,
        currentAddress: activeAddress,
        recognizedPackagesList,
    });

    if (!summary) return null;

    const { digest } = summary;

    return (
        <div className="flex h-full w-full flex-col justify-between">
            <TransactionReceipt
                txn={txn}
                summary={summary}
                activeAddress={activeAddress}
                renderExplorerLink={ExplorerLinkHelper}
            />
            <div className="flex flex-row space-x-xs pt-sm">
                <div className="flex w-full [&_a]:w-full">
                    <ExplorerLink transactionID={digest ?? ''} type={ExplorerLinkType.Transaction}>
                        <ViewTxnOnExplorerButton digest={digest} />
                    </ExplorerLink>
                </div>
                <div className="self-center">
                    <OutlinedCopyButton
                        textToCopy={digest ?? ''}
                        onCopySuccess={() =>
                            toast.success('Transaction digest copied to clipboard')
                        }
                    />
                </div>
            </div>
        </div>
    );
}
