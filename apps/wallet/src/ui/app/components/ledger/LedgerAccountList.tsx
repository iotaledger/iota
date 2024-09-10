// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Table,
    TableBody,
    TableBodyRow,
    TableCell,
    TableCellType,
    TableHeader,
    TableHeaderCell,
    TableHeaderRow,
} from '@iota/apps-ui-kit';
import { type DerivedLedgerAccount } from './useDeriveLedgerAccounts';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useBalance, useFormatCoin, useResolveIotaNSName } from '@iota/core';

export type SelectableLedgerAccount = DerivedLedgerAccount & {
    isSelected: boolean;
};

interface LedgerAccountListProps {
    accounts: SelectableLedgerAccount[];
    onAccountClick: (account: SelectableLedgerAccount) => void;
    selectAll: () => void;
}

export function LedgerAccountList({ accounts, onAccountClick, selectAll }: LedgerAccountListProps) {
    const headersData = [
        { label: 'Address', columnKey: 1 },
        { label: '', columnKey: 2 },
    ];

    const rowsData = accounts.map((account) => {
        const { data: coinBalance } = useBalance(account.address);
        const { data: domainName } = useResolveIotaNSName(account.address);
        const [totalAmount, totalAmountSymbol] = useFormatCoin(
            coinBalance?.totalBalance ?? 0,
            IOTA_TYPE_ARG,
        );

        return [
            {
                label: domainName ?? formatAddress(account.address),
            },
            {
                label: `${totalAmount} ${totalAmountSymbol}`,
            },
        ];
    });

    return (
        <Table
            hasCheckboxColumn={true}
            onRowCheckboxChange={(_, index) => {
                onAccountClick(accounts[index]);
            }}
            onHeaderCheckboxChange={() => selectAll()}
            rowIndexes={rowsData.map((_, i) => i)}
        >
            <TableHeader>
                <TableHeaderRow>
                    {headersData.map((header, index) => (
                        <TableHeaderCell key={index} {...header} />
                    ))}
                </TableHeaderRow>
            </TableHeader>
            <TableBody>
                {rowsData.map((row, rowIndex) => (
                    <TableBodyRow key={rowIndex} rowIndex={rowIndex}>
                        {row.map((cell, cellIndex) => (
                            <TableCell key={cellIndex} type={TableCellType.Text} {...cell} />
                        ))}
                    </TableBodyRow>
                ))}
            </TableBody>
        </Table>
    );
}
