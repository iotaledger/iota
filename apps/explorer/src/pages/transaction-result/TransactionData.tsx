// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages, useTransactionSummary } from '@iota/core';
import {
    type ProgrammableTransaction,
    type IotaTransactionBlockResponse,
} from '@iota/iota-sdk/client';
import { GasBreakdown } from '~/components';
import { InputsCard } from '~/pages/transaction-result/programmable-transaction-view/InputsCard';
import { TransactionsCard } from '~/pages/transaction-result/programmable-transaction-view/TransactionsCard';

interface TransactionDataProps {
    transaction: IotaTransactionBlockResponse;
}

export function TransactionData({ transaction }: TransactionDataProps): JSX.Element {
    const recognizedPackagesList = useRecognizedPackages();
    const summary = useTransactionSummary({
        transaction,
        recognizedPackagesList,
    });

    const transactionKindName = transaction.transaction?.data.transaction.kind;

    const isProgrammableTransaction = transactionKindName === 'ProgrammableTransaction';

    const programmableTxn = transaction.transaction!.data.transaction as ProgrammableTransaction;

    return (
        <div className="flex w-full flex-col gap-3 pl-1 pr-2 md:gap-6">
            <section className="flex w-full flex-1 flex-col gap-3  md:gap-6">
                {isProgrammableTransaction && (
                    <div data-testid="inputs-card">
                        <InputsCard inputs={programmableTxn.inputs} />
                    </div>
                )}
            </section>

            {isProgrammableTransaction && (
                <section className="md:min-w-transactionColumn flex w-full flex-1 flex-col gap-3 md:gap-6">
                    <div data-testid="transactions-card">
                        <TransactionsCard transactions={programmableTxn.transactions} />
                    </div>
                    <div data-testid="gas-breakdown">
                        <GasBreakdown summary={summary} />
                    </div>
                </section>
            )}
        </div>
    );
}
