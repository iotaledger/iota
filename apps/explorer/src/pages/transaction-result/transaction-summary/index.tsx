// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTransactionSummary } from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota.js/client';

import { BalanceChanges } from './BalanceChanges';
import { ObjectChanges } from './ObjectChanges';
import { UpgradedSystemPackages } from './UpgradedSystemPackages';
import { useRecognizedPackages } from '~/hooks/useRecognizedPackages';

interface TransactionSummaryProps {
    transaction: IotaTransactionBlockResponse;
}

export function TransactionSummary({ transaction }: TransactionSummaryProps) {
    const recognizedPackagesList = useRecognizedPackages();
    const summary = useTransactionSummary({
        transaction,
        recognizedPackagesList,
    });

    const transactionKindName = transaction.transaction?.data.transaction.kind;

    const balanceChanges = summary?.balanceChanges;
    const objectSummary = summary?.objectSummary;
    const upgradedSystemPackages = summary?.upgradedSystemPackages;

    return (
        <div className="flex flex-wrap gap-4 md:gap-8">
            {balanceChanges && transactionKindName === 'ProgrammableTransaction' && (
                <BalanceChanges changes={balanceChanges} />
            )}
            {upgradedSystemPackages && <UpgradedSystemPackages data={upgradedSystemPackages} />}
            {objectSummary && <ObjectChanges objectSummary={objectSummary} />}
        </div>
    );
}
