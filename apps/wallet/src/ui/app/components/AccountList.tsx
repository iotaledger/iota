// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
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
    const headersData = [
        { label: 'Address', columnKey: 1 },
        { label: '', columnKey: 2 },
    ];

    const selectedRowIndexes = accounts.reduce((set, acc, i) => {
        if (selectedAccounts.has(acc.address)) {
            set.add(i);
        }
        return set;
    }, new Set<number>());

    return (
        <Table selectedRowIndexes={selectedRowIndexes} rowIndexes={accounts.map((_, i) => i)}>
            <TableHeader>
                <TableRow leading={<TableHeaderCheckbox onCheckboxChange={() => selectAll()} />}>
                    {headersData.map((header, index) => (
                        <TableHeaderCell key={index} {...header} />
                    ))}
                </TableRow>
            </TableHeader>
            <TableBody>
                {accounts.map((account, rowIndex) => (
                    <AccountRow
                        key={account.address}
                        account={account}
                        rowIndex={rowIndex}
                        onAccountClick={onAccountClick}
                    />
                ))}
            </TableBody>
        </Table>
    );
}

function AccountRow<A extends { address: string }>({
    account,
    rowIndex,
    onAccountClick,
}: {
    account: A;
    rowIndex: number;
    onAccountClick: (account: A, checked: boolean) => void;
}) {
    const { data: coinBalance } = useBalance(account.address);
    const [totalAmount, totalAmountSymbol] = useFormatCoin({
        balance: coinBalance?.totalBalance ?? 0,
    });
    const cells = [formatAddress(account.address), `${totalAmount} ${totalAmountSymbol}`];

    return (
        <TableRow
            leading={
                <TableRowCheckbox
                    rowIndex={rowIndex}
                    onCheckboxChange={(checked) => onAccountClick(account, checked)}
                />
            }
        >
            {cells.map((cell, cellIndex) => (
                <TableCellBase key={cellIndex}>
                    <TableCellText>{cell}</TableCellText>
                </TableCellBase>
            ))}
        </TableRow>
    );
}
