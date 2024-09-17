// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTransactionData } from '_src/ui/app/hooks';
import { type TransactionBlock } from '@iota/iota-sdk/transactions';

import { Command } from './Command';
import { Input } from './Input';
import { Collapsible } from '_src/ui/app/shared/collapse';
import {
    ButtonSegment,
    ButtonSegmentType,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { useEffect, useState } from 'react';
import { Alert, Loading } from '_src/ui/app/components';

interface TransactionDetailsProps {
    sender?: string;
    transaction: TransactionBlock;
}

enum DetailsCategory {
    Transactions = 'Transactions',
    Inputs = 'Inputs',
}
const DETAILS_CATEGORIES = [
    {
        label: 'Transactions',
        value: DetailsCategory.Transactions,
    },
    {
        label: 'Inputs',
        value: DetailsCategory.Inputs,
    },
];

export function TransactionDetails({ sender, transaction }: TransactionDetailsProps) {
    const [selectedDetailsCategory, setSelectedDetailsCategory] = useState<DetailsCategory | null>(
        null,
    );
    const { data: transactionData, isPending, isError } = useTransactionData(sender, transaction);
    useEffect(() => {
        if (transactionData) {
            const defaultCategory =
                transactionData.transactions.length > 0
                    ? DetailsCategory.Transactions
                    : transactionData.inputs.length > 0
                      ? DetailsCategory.Inputs
                      : null;

            if (defaultCategory) {
                setSelectedDetailsCategory(defaultCategory);
            }
        }
    }, [transactionData]);

    if (transactionData?.transactions.length === 0 && transactionData.inputs.length === 0) {
        return null;
    }
    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm overflow-hidden rounded-xl">
                <Collapsible
                    hideBorder
                    defaultOpen
                    render={() => <Title size={TitleSize.Small} title="Transaction Details" />}
                >
                    <SegmentedButton type={SegmentedButtonType.Transparent}>
                        {DETAILS_CATEGORIES.map(({ label, value }) => (
                            <ButtonSegment
                                type={ButtonSegmentType.Underlined}
                                key={value}
                                onClick={() => setSelectedDetailsCategory(value)}
                                label={label}
                                selected={selectedDetailsCategory === value}
                                disabled={
                                    DetailsCategory.Transactions === value
                                        ? transactionData?.transactions.length === 0
                                        : DetailsCategory.Inputs === value
                                          ? transactionData?.inputs.length === 0
                                          : false
                                }
                            />
                        ))}
                    </SegmentedButton>
                    <Loading loading={isPending}>
                        {isError ? (
                            <Alert>
                                <div>
                                    <strong>Couldn't gather data</strong>
                                </div>
                            </Alert>
                        ) : null}
                        <div className="flex flex-col p-md">
                            {selectedDetailsCategory === DetailsCategory.Transactions &&
                                !!transactionData?.transactions.length && (
                                    <div className="flex flex-col gap-md">
                                        {transactionData?.transactions.map((command, index) => (
                                            <Command key={index} command={command} />
                                        ))}
                                    </div>
                                )}
                            {selectedDetailsCategory === DetailsCategory.Inputs &&
                                !!transactionData?.inputs.length && (
                                    <div className="flex flex-col gap-md">
                                        {transactionData?.inputs.map((input, index) => (
                                            <Input key={index} input={input} />
                                        ))}
                                    </div>
                                )}
                        </div>
                    </Loading>
                </Collapsible>
            </div>
        </Panel>
    );
}
