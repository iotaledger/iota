// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTransactionSummary, useRecognizedPackages } from '@iota/core';
import { type IotaTransactionBlockResponse, type Network } from '@iota/iota-sdk/client';
import { BalanceChanges } from './BalanceChanges';
import { ObjectChanges } from './ObjectChanges';
import { UpgradedSystemPackages } from './UpgradedSystemPackages';
import { useNetwork } from '~/hooks';

interface TransactionSummaryProps {
    transaction: IotaTransactionBlockResponse;
}

export function TransactionSummary({ transaction }: TransactionSummaryProps): JSX.Element {
    const [network] = useNetwork();
    const recognizedPackagesList = useRecognizedPackages(network as Network);
    const summary = useTransactionSummary({
        transaction,
        recognizedPackagesList,
    });

    const transactionKindName = transaction.transaction?.data.transaction.kind;

    const balanceChanges = summary?.balanceChanges;
    const objectSummary = summary?.objectSummary;
    const upgradedSystemPackages = summary?.upgradedSystemPackages;

    return (
        <div className="flex flex-wrap gap-lg px-md--rs py-md md:py-sm">
            {balanceChanges && transactionKindName === 'ProgrammableTransaction' && (
                <BalanceChanges changes={balanceChanges} />
            )}
            {upgradedSystemPackages && <UpgradedSystemPackages data={upgradedSystemPackages} />}
            {objectSummary && <ObjectChanges objectSummary={objectSummary} />}
        </div>
    );
}
