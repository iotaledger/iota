// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { AccountsFormType, useAccountsFormContext, VerifyPasswordModal } from '_components';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useCreateAccountsMutation } from '_src/ui/app/hooks/useCreateAccountMutation';
import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Button, ButtonSize, ButtonType, Dropdown, ListItem } from '@iota/apps-ui-kit';
import { Add, MoreHoriz, TriangleDown } from '@iota/ui-icons';
import { OutsideClickHandler } from '_components/OutsideClickHandler';
import { AccountGroupItem } from '_pages/accounts/manage/AccountGroupItem';
import { Collapsible } from '_app/shared/collapse';

const ACCOUNT_TYPE_TO_LABEL: Record<AccountType, string> = {
    [AccountType.MnemonicDerived]: 'Mnemonic',
    [AccountType.SeedDerived]: 'Seed',
    [AccountType.PrivateKeyDerived]: 'Private Key',
    [AccountType.LedgerDerived]: 'Ledger',
};
const ACCOUNTS_WITH_ENABLED_BALANCE_FINDER: AccountType[] = [
    AccountType.MnemonicDerived,
    AccountType.SeedDerived,
    AccountType.LedgerDerived,
];

export function getGroupTitle(aGroupAccount: SerializedUIAccount) {
    return ACCOUNT_TYPE_TO_LABEL[aGroupAccount?.type] || '';
}

export function AccountGroup({
    accounts,
    type,
    accountSourceID,
}: {
    accounts: SerializedUIAccount[];
    type: AccountType;
    accountSourceID?: string;
}) {
    const [isDropdownOpen, setDropdownOpen] = useState(false);
    const navigate = useNavigate();
    const createAccountMutation = useCreateAccountsMutation();
    const isMnemonicDerivedGroup = type === AccountType.MnemonicDerived;
    const isSeedDerivedGroup = type === AccountType.SeedDerived;
    const [accountsFormValues, setAccountsFormValues] = useAccountsFormContext();
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const { data: accountSources } = useAccountSources();
    const accountSource = accountSources?.find(({ id }) => id === accountSourceID);

    async function handleAdd(e: React.MouseEvent<HTMLButtonElement>) {
        if (!accountSource) return;

        // prevent the collapsible from closing when clicking the "new" button
        e.stopPropagation();
        const accountsFormType = isMnemonicDerivedGroup
            ? AccountsFormType.MnemonicSource
            : AccountsFormType.SeedSource;
        setAccountsFormValues({
            type: accountsFormType,
            sourceID: accountSource.id,
        });
        if (accountSource.isLocked) {
            setPasswordModalVisible(true);
        } else {
            createAccountMutation.mutate({
                type: accountsFormType,
            });
        }
    }

    function handleBalanceFinder() {
        navigate(`/accounts/manage/accounts-finder/${accountSourceID}`);
    }

    function handleExportMnemonic() {
        navigate(`../export/passphrase/${accountSource!.id}`);
    }

    function handleExportSeed() {
        navigate(`../export/seed/${accountSource!.id}`);
    }

    const dropdownVisibility = {
        showBalanceFinder: ACCOUNTS_WITH_ENABLED_BALANCE_FINDER.includes(type),
        showExportMnemonic: isMnemonicDerivedGroup && accountSource,
        showExportSeed: isSeedDerivedGroup && accountSource,
    };
    const showMoreButton = Object.values(dropdownVisibility).some((v) => v);

    return (
        <div className="relative overflow-visible">
            <Collapsible
                defaultOpen
                hideArrow
                hideBorder
                render={({ isOpen }) => (
                    <div className="relative flex w-full items-center justify-between gap-1 py-2 pl-1 pr-sm">
                        <div className="flex items-center gap-1">
                            <TriangleDown
                                className={`${isOpen ? 'rotate-0 transition-transform ease-linear' : '-rotate-90 transition-transform ease-linear'} h-5 w-5 text-neutral-60`}
                            />
                            <div className="text-title-md">{getGroupTitle(accounts[0])}</div>
                        </div>
                        <div className="flex items-center gap-1">
                            {(isMnemonicDerivedGroup || isSeedDerivedGroup) && accountSource ? (
                                <Button
                                    size={ButtonSize.Small}
                                    type={ButtonType.Ghost}
                                    onClick={handleAdd}
                                    icon={<Add className="h-5 w-5 text-neutral-10" />}
                                />
                            ) : null}
                            {showMoreButton && (
                                <div className="relative">
                                    <Button
                                        size={ButtonSize.Small}
                                        type={ButtonType.Ghost}
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            setDropdownOpen(true);
                                        }}
                                        icon={<MoreHoriz className="h-5 w-5 text-neutral-10" />}
                                    />
                                </div>
                            )}
                        </div>
                    </div>
                )}
            >
                {accounts.map((account, index) => (
                    <AccountGroupItem
                        key={account.id}
                        account={account}
                        isLast={index === accounts.length - 1}
                    />
                ))}
            </Collapsible>
            <div
                className={`absolute right-0 top-0 z-[100] bg-white ${isDropdownOpen ? '' : 'hidden'}`}
            >
                <OutsideClickHandler onOutsideClick={() => setDropdownOpen(false)}>
                    <Dropdown>
                        {dropdownVisibility.showBalanceFinder && (
                            <ListItem hideBottomBorder onClick={handleBalanceFinder}>
                                Balance finder
                            </ListItem>
                        )}

                        {dropdownVisibility.showExportMnemonic && (
                            <ListItem hideBottomBorder onClick={handleExportMnemonic}>
                                Export Mnemonic
                            </ListItem>
                        )}
                        {dropdownVisibility.showExportSeed && (
                            <ListItem hideBottomBorder onClick={handleExportSeed}>
                                Export Seed
                            </ListItem>
                        )}
                    </Dropdown>
                </OutsideClickHandler>
            </div>
            {isPasswordModalVisible ? (
                <VerifyPasswordModal
                    open
                    onVerify={async (password) => {
                        if (accountsFormValues.current) {
                            await createAccountMutation.mutateAsync({
                                type: accountsFormValues.current.type,
                                password,
                            });
                        }
                        setPasswordModalVisible(false);
                    }}
                    onClose={() => setPasswordModalVisible(false)}
                />
            ) : null}
        </div>
    );
}
