// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { Collapsible } from '_src/ui/app/shared/collapse';
import { Filter16, Plus12 } from '@iota/icons';
import * as ToggleGroup from '@radix-ui/react-toggle-group';
import { useMemo, useState } from 'react';

import { useAccountGroups } from '../../hooks/useAccountGroups';
import { useActiveAccount } from '../../hooks/useActiveAccount';
import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { Heading } from '../../shared/heading';
import { AccountListItem } from './AccountListItem';
import { FooterLink } from './FooterLink';
import { useUnlockAccount } from './UnlockAccountContext';
import { type SerializedUIAccount } from '_src/background/accounts/Account';

export function AccountsList() {
    const accountGroups = useAccountGroups();
    const accounts = accountGroups.list();
    const activeAccount = useActiveAccount();
    const backgroundClient = useBackgroundClient();
    const [isSwitchToAccountOpen, setIsSwitchToAccountOpen] = useState(false);
    const { unlockAccount, lockAccount } = useUnlockAccount();

    const otherAccounts = useMemo(
        () => accounts.filter((a) => a.id !== activeAccount?.id) || [],
        [accounts, activeAccount?.id],
    );
    function handleLockAndUnlockClick(account: SerializedUIAccount) {
        if (account.isLocked) {
            unlockAccount(account);
        } else {
            lockAccount(account);
        }
    }
    const handleSelectAccount = async (accountID: string) => {
        const account = accounts?.find((a) => a.id === accountID);
        if (!account) return;
        if (accountID !== activeAccount?.id) {
            ampli.switchedAccount({
                toAccountType: account.type,
            });
            await backgroundClient.selectAccount(accountID);
            setIsSwitchToAccountOpen(false);
        }
    };
    if (!accounts || !activeAccount) return null;

    return (
        <div className="border-hero/10 flex w-full select-none flex-col gap-5 rounded-xl border border-solid bg-gradients-graph-cards p-4">
            <Heading variant="heading5" weight="semibold" color="steel-darker">
                Accounts
            </Heading>

            <ToggleGroup.Root
                asChild
                value={activeAccount.id}
                type="single"
                onValueChange={handleSelectAccount}
            >
                <>
                    <Collapsible defaultOpen title="Current" shade="darker">
                        <ToggleGroup.Item asChild value={activeAccount.id}>
                            <div>
                                <AccountListItem
                                    account={activeAccount}
                                    editable
                                    showLock
                                    onLockAccountClick={() =>
                                        handleLockAndUnlockClick(activeAccount)
                                    }
                                    onUnlockAccountClick={() =>
                                        handleLockAndUnlockClick(activeAccount)
                                    }
                                />
                            </div>
                        </ToggleGroup.Item>
                    </Collapsible>

                    {otherAccounts.length ? (
                        <Collapsible
                            isOpen={isSwitchToAccountOpen}
                            onOpenChange={setIsSwitchToAccountOpen}
                            title="Switch To"
                            shade="darker"
                        >
                            <div className="flex flex-col gap-3">
                                {otherAccounts.map((account) => {
                                    return (
                                        <ToggleGroup.Item
                                            asChild
                                            key={account.id}
                                            value={account.id}
                                        >
                                            <div>
                                                <AccountListItem
                                                    account={account}
                                                    showLock
                                                    onLockAccountClick={() =>
                                                        handleLockAndUnlockClick(account)
                                                    }
                                                    onUnlockAccountClick={() =>
                                                        handleLockAndUnlockClick(account)
                                                    }
                                                />
                                            </div>
                                        </ToggleGroup.Item>
                                    );
                                })}
                            </div>
                        </Collapsible>
                    ) : null}
                </>
            </ToggleGroup.Root>

            <div className="flex justify-between">
                <FooterLink
                    color="steelDarker"
                    icon={<Filter16 />}
                    to="/accounts/manage"
                    text="Manage"
                />
                <FooterLink
                    color="steelDarker"
                    icon={<Plus12 />}
                    to="/accounts/add-account"
                    text="Add"
                />
            </div>
        </div>
    );
}
