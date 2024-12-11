// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages } from '_src/ui/app/hooks/useRecognizedPackages';
import {
    useTransactionSummary,
    TransactionReceipt,
    ExplorerLinkType,
    ViewTxnOnExplorerButton,
} from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { CardType } from '@iota/apps-ui-kit';
import { ValidatorLogo } from '../../staking/validators/ValidatorLogo';
import { ExplorerLinkHelper } from '../ExplorerLinkHelper';
import ExplorerLink from '../explorer-link';

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
                renderValidatorLogo={({ address, showActiveStatus, activeEpoch, isSelected }) => (
                    <ValidatorLogo
                        validatorAddress={address}
                        showActiveStatus={showActiveStatus}
                        activeEpoch={activeEpoch}
                        type={isSelected ? CardType.Filled : CardType.Outlined}
                    />
                )}
            />
            <div className="pt-sm">
                <ExplorerLink transactionID={digest ?? ''} type={ExplorerLinkType.Transaction}>
                    <ViewTxnOnExplorerButton digest={digest} />
                </ExplorerLink>
            </div>
        </div>
    );
}
