// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTransactionData } from '_hooks';
import { type Transaction } from '@iota/iota-sdk/transactions';
import { Command } from './Command';
import { Input } from './Input';
import { Collapsible } from '@iota/core';
import {
    ButtonSegment,
    ButtonSegmentType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { useEffect, useState } from 'react';
import { Loading } from '_src/ui/app/components';
import { Warning } from '@iota/apps-ui-icons';

interface TransactionDetailsProps {
    sender?: string;
    transaction: Transaction;
}

enum DetailsCategory {
    Commands = 'Commands',
    Inputs = 'Inputs',
}
const DETAILS_CATEGORIES = [
    {
        label: 'Commands',
        value: DetailsCategory.Commands,
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
    const {
        data: transactionData,
        isPending,
        isError,
        error,
    } = useTransactionData(sender, transaction);
    useEffect(() => {
        if (transactionData) {
            const defaultCategory =
                transactionData.commands.length > 0
                    ? DetailsCategory.Commands
                    : transactionData.inputs.length > 0
                      ? DetailsCategory.Inputs
                      : null;

            if (defaultCategory) {
                setSelectedDetailsCategory(defaultCategory);
            }
        }
    }, [transactionData]);

    if (transactionData?.commands.length === 0 && transactionData.inputs.length === 0) {
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
                                    DetailsCategory.Commands === value
                                        ? transactionData?.commands.length === 0
                                        : DetailsCategory.Inputs === value
                                          ? transactionData?.inputs.length === 0
                                          : false
                                }
                            />
                        ))}
                    </SegmentedButton>
                    <Loading loading={isPending}>
                        {isError ? (
                            <InfoBox
                                type={InfoBoxType.Error}
                                title={error?.message ?? "Couldn't gather data"}
                                icon={<Warning />}
                                style={InfoBoxStyle.Elevated}
                            />
                        ) : null}
                        <div className="flex flex-col p-md">
                            {selectedDetailsCategory === DetailsCategory.Commands &&
                                !!transactionData?.commands.length && (
                                    <div className="flex flex-col gap-md">
                                        {transactionData?.commands.map((command, index) => (
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
