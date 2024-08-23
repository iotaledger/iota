// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { AccountsFormType, useAccountsFormContext, VerifyPasswordModal } from '_components';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useCreateAccountsMutation } from '_src/ui/app/hooks/useCreateAccountMutation';
import { Button, ButtonSize, ButtonType, Dropdown, ListItem } from '@iota/apps-ui-kit';
import { Add, MoreHoriz } from '@iota/ui-icons';
import { Collapse, CollapseBody, CollapseHeader } from './Collapse';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { OutsideClickHandler } from '_pages/accounts/manage/OutsideClickHandler';
import { AccountGroupItem } from '_pages/accounts/manage/AccountGroupItem';

const ACCOUNT_TYPE_TO_LABEL: Record<AccountType, string> = {
    [AccountType.MnemonicDerived]: 'Passphrase Derived',
    [AccountType.SeedDerived]: 'Seed Derived',
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

    function handleExportPassphrase() {
        navigate(`../export/passphrase/${accountSource!.id}`);
    }

    function handleExportSeed() {
        navigate(`../export/seed/${accountSource!.id}`);
    }

    return (
        <div className="relative overflow-visible">
            <Collapse defaultOpen>
                <CollapseHeader title={getGroupTitle(accounts[0])}>
                    <div className="flex items-center gap-1">
                        {(isMnemonicDerivedGroup || isSeedDerivedGroup) && accountSource ? (
                            <Button
                                size={ButtonSize.Small}
                                type={ButtonType.Ghost}
                                onClick={handleAdd}
                                icon={<Add className="h-5 w-5 text-neutral-10" />}
                            />
                        ) : null}
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
                    </div>
                </CollapseHeader>
                <CollapseBody>
                    {accounts.map((account, index) => (
                        <AccountGroupItem
                            account={account}
                            isLast={index === accounts.length - 1}
                        />
                    ))}
                </CollapseBody>
                <div
                    className={`absolute right-0 top-0 z-[100] bg-white ${isDropdownOpen ? '' : 'hidden'}`}
                >
                    <OutsideClickHandler onOutsideClick={() => setDropdownOpen(false)}>
                        <Dropdown>
                            {ACCOUNTS_WITH_ENABLED_BALANCE_FINDER.includes(type) && (
                                <ListItem hideBottomBorder onClick={handleBalanceFinder}>
                                    Balance finder
                                </ListItem>
                            )}

                            {isMnemonicDerivedGroup && accountSource && (
                                <ListItem hideBottomBorder onClick={handleExportPassphrase}>
                                    Export Passphrase
                                </ListItem>
                            )}
                            {isSeedDerivedGroup && accountSource && (
                                <ListItem hideBottomBorder onClick={handleExportSeed}>
                                    Export Seed
                                </ListItem>
                            )}
                        </Dropdown>
                    </OutsideClickHandler>
                </div>
            </Collapse>
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
