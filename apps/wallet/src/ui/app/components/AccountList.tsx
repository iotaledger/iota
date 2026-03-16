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

    const selectedRowIndexes = accounts.reduce((set, acc, i) => {
        if (selectedAccounts.has(acc.address) || existingAddresses.has(acc.address)) {
            set.add(i);
        }
        return set;
    }, new Set<number>());

    const rowIndexes = accounts.map((_, i) => i);

    return (
        <Table selectedRowIndexes={selectedRowIndexes} rowIndexes={rowIndexes}>
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
                        isExisting={existingAddresses.has(accounts[rowIndex].address)}
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
    isExisting,
}: {
    account: A;
    rowIndex: number;
    onAccountClick: (account: A, checked: boolean) => void;
    isExisting: boolean;
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
                    isDisabled={isExisting}
                    onCheckboxChange={(checked) => {
                        if (isExisting) return;
                        onAccountClick(account, checked);
                    }}
                />
            }
        >
            {cells.map((cell, cellIndex) => (
                <TableCellBase key={cellIndex}>
                    <TableCellText>
                        {cellIndex === 0 ? <span data-amp-mask>{cell}</span> : cell}
                    </TableCellText>
                </TableCellBase>
            ))}
        </TableRow>
    );
}
