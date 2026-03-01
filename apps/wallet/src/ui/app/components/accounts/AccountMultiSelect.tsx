// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountItemApproveConnection, SelectAllButton } from '_components';
import { type SerializedUIAccount } from '_src/background/accounts/account';
import * as ToggleGroup from '@radix-ui/react-toggle-group';

interface AccountMultiSelectProps {
    accounts: SerializedUIAccount[];
    selectedAccountIDs: string[];
    onChange: (value: string[]) => void;
}

function AccountMultiSelect({ accounts, selectedAccountIDs, onChange }: AccountMultiSelectProps) {
    return (
        <ToggleGroup.Root
            value={selectedAccountIDs}
            onValueChange={onChange}
            type="multiple"
            className="flex flex-col gap-3"
        >
            {accounts.map((account) => (
                <ToggleGroup.Item key={account.id} asChild value={account.id}>
                    <div>
                        <AccountItemApproveConnection
                            account={account}
                            selected={selectedAccountIDs.includes(account.id)}
                        />
                    </div>
                </ToggleGroup.Item>
            ))}
        </ToggleGroup.Root>
    );
}

export function AccountMultiSelectWithControls({
    selectedAccountIDs,
    accounts,
    onChange,
}: AccountMultiSelectProps) {
    return (
        <div className="flex flex-col gap-3 [&>button]:border-none">
            <AccountMultiSelect
                selectedAccountIDs={selectedAccountIDs}
                accounts={accounts}
                onChange={onChange}
            />

            {accounts.length > 1 ? (
                <SelectAllButton
                    accountIds={accounts.map((account) => account.id)}
                    selectedAccountIds={selectedAccountIDs}
                    onChange={onChange}
                />
            ) : null}
        </div>
    );
}
