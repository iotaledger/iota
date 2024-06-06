// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LedgerAccountRow } from './LedgerAccountRow';
import { type DerivedLedgerAccount } from './useDeriveLedgerAccounts';

export type SelectableLedgerAccount = DerivedLedgerAccount & {
    isSelected: boolean;
};

type LedgerAccountListProps = {
    accounts: SelectableLedgerAccount[];
    onAccountClick: (account: SelectableLedgerAccount) => void;
};

export function LedgerAccountList({ accounts, onAccountClick }: LedgerAccountListProps) {
    return (
        <ul className="m-0 list-none p-0">
            {accounts.map((account) => (
                <li className="pb-2 pt-2 first:pt-1" key={account.address}>
                    <button
                        className="w-full cursor-pointer appearance-none border-0 bg-transparent p-0"
                        onClick={() => {
                            onAccountClick(account);
                        }}
                    >
                        <LedgerAccountRow
                            isSelected={account.isSelected}
                            address={account.address}
                        />
                    </button>
                </li>
            ))}
        </ul>
    );
}
