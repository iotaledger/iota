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
import { type DerivedLedgerAccount } from './useDeriveLedgerAccounts';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useBalance, useFormatCoin } from '@iota/core';

interface LedgerAccountListProps {
    accounts: DerivedLedgerAccount[];
    selectedAccounts: Set<string>;
    onAccountClick: (account: DerivedLedgerAccount, checked: boolean) => void;
    selectAll: () => void;
}

export function LedgerAccountList({
    accounts,
    selectedAccounts,
    onAccountClick,
    selectAll,
}: LedgerAccountListProps) {
    const headersData = [
        { label: 'Address', columnKey: 1 },
        { label: '', columnKey: 2 },
    ];

    const rowsData = accounts.map((account) => {
        const { data: coinBalance } = useBalance(account.address);
        const [totalAmount, totalAmountSymbol] = useFormatCoin(
            coinBalance?.totalBalance ?? 0,
            IOTA_TYPE_ARG,
        );

        return [formatAddress(account.address), `${totalAmount} ${totalAmountSymbol}`];
    });

    const selectedRowIndexes = accounts.reduce((set, acc, i) => {
        if (selectedAccounts.has(acc.address)) {
            set.add(i);
        }
        return set;
    }, new Set<number>());

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
                {rowsData.map((row, rowIndex) => (
                    <TableRow
                        key={rowIndex}
                        leading={
                            <TableRowCheckbox
                                rowIndex={rowIndex}
                                onCheckboxChange={(checked) =>
                                    onAccountClick(accounts[rowIndex], checked)
                                }
                            />
                        }
                    >
                        {row.map((cell, cellIndex) => (
                            <TableCellBase key={cellIndex}>
                                <TableCellText>{cell}</TableCellText>
                            </TableCellBase>
                        ))}
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}
