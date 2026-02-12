// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    LoadingIndicator,
    Table,
    TableBody,
    TableCellBase,
    TableCellText,
    TableHeader,
    TableHeaderCell,
    TableHeaderCheckbox,
    TableRow,
    TableRowCheckbox,
} from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useBalance, useFormatCoin } from '@iota/core';
import { useAccounts } from '_hooks';

interface AccountListProps<A> {
    accounts: A[];
    selectedAccounts: Set<string>;
    onAccountClick: (account: A, checked: boolean) => void;
    selectAll: () => void;
}

export function AccountList<A extends { address: string }>({
    accounts,
    selectedAccounts,
    onAccountClick,
    selectAll,
}: AccountListProps<A>) {
    const { data: existingAccounts } = useAccounts();

    const existingAddresses = new Set((existingAccounts ?? []).map((acc) => acc.address));
    const headersData = [
        { label: 'Address', columnKey: 1 },
        { label: '', columnKey: 2 },
    ];

    const rowsData = accounts.map((account) => {
        const { data: coinBalance } = useBalance(account.address);
        const [totalAmount, totalAmountSymbol] = useFormatCoin({
            balance: coinBalance?.totalBalance ?? 0,
        });

        return [formatAddress(account.address), `${totalAmount} ${totalAmountSymbol}`];
    });

    const selectedRowIndexes = accounts.reduce((set, acc, i) => {
        if (selectedAccounts.has(acc.address)) {
            set.add(i);
        }
        return set;
    }, new Set<number>());

    function LedgerViewLoading() {
        return (
            <div className="flex h-full w-full flex-row items-center justify-center gap-x-sm">
                <LoadingIndicator />
                <span className="text-title-lg text-iota-neutral-10 dark:text-iota-neutral-92">
                    Looking for Accounts...
                </span>
            </div>
        );
    }

    return (
        <Table selectedRowIndexes={selectedRowIndexes} rowIndexes={rowsData.map((_, i) => i)}>
            <TableHeader>
                <TableRow leading={<TableHeaderCheckbox onCheckboxChange={() => selectAll()} />}>
                    {headersData.map((header, index) => (
                        <TableHeaderCell key={index} {...header} />
                    ))}
                </TableRow>
            </TableHeader>
            <TableBody>
                {rowsData.map((row, rowIndex) => {
                    const isExisting = existingAddresses.has(accounts[rowIndex].address);
                    return (
                        <TableRow
                            key={rowIndex}
                            leading={
                                <TableRowCheckbox
                                    rowIndex={rowIndex}
                                    isDisabled={isExisting}
                                    onCheckboxChange={(checked) => {
                                        if (isExisting) return;
                                        onAccountClick(accounts[rowIndex], checked);
                                    }}
                                />
                            }
                        >
                            {row.map((cell, cellIndex) => (
                                <TableCellBase key={cellIndex}>
                                    <TableCellText>{cell}</TableCellText>
                                </TableCellBase>
                            ))}
                        </TableRow>
                    );
                })}
            </TableBody>
        </Table>
    );
}
