// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { AccountsFormType, useAccountsFormContext, useSourceFlow } from '_components';
import { useAccountSources, useActiveAccount, useCreateAccountsMutation } from '_hooks';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import clsx from 'clsx';
import { AmpliSourceFlow } from '_src/shared/analytics';
import { Button, ButtonSize, ButtonType, Divider, Dropdown, ListItem } from '@iota/apps-ui-kit';
import { Add, ArrowDown, MoreHoriz, TriangleDown } from '@iota/apps-ui-icons';
import { OutsideClickHandler } from '_components/OutsideClickHandler';
import { AccountGroupItem } from '_pages/accounts/manage/AccountGroupItem';
import { useFeature } from '@growthbook/growthbook-react';
import { Feature, Collapsible } from '@iota/core';
import { isLegacyAccount } from '_src/background/accounts/isLegacyAccount';
import { parseDerivationPath } from '_src/background/account-sources/bip44Path';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/mnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/seedAccount';
import { isKeystoneAccountSerializedUI } from '_src/background/accounts/keystoneAccount';
import { AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';

const ACCOUNT_TYPE_TO_LABEL: Record<AccountType, string> = {
    [AccountType.MnemonicDerived]: 'Mnemonic',
    [AccountType.SeedDerived]: 'Seed',
    [AccountType.PrivateKeyDerived]: 'Private Key',
    [AccountType.PasskeyDerived]: 'Passkey',
    [AccountType.LedgerDerived]: 'Ledger',
    [AccountType.KeystoneDerived]: 'Keystone',
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
    isLast,
    outerRef,
}: {
    accounts: SerializedUIAccount[];
    type: AccountType;
    accountSourceID?: string;
    isLast: boolean;
    outerRef?: React.RefObject<HTMLDivElement>;
}) {
    const [isDropdownOpen, setDropdownOpen] = useState(false);
    const [isCollapsibleGroupOpen, setIsCollapsibleGroupOpen] = useState(true);
    const navigate = useNavigate();
    const activeAccount = useActiveAccount();
    const createAccountsMutation = useCreateAccountsMutation();
    const isMnemonicDerivedGroup = type === AccountType.MnemonicDerived;
    const isSeedDerivedGroup = type === AccountType.SeedDerived;
    const [, setAccountsFormValues] = useAccountsFormContext();
    const { setSourceFlow } = useSourceFlow();
    const { data: accountSources } = useAccountSources();
    const accountSource = accountSources?.find(({ id }) => id === accountSourceID);

    async function handleAdd(e: React.MouseEvent<HTMLButtonElement>) {
        if (!accountSource) return;

        setIsCollapsibleGroupOpen(true);

        // prevent the collapsible from closing when clicking the "new" button
        e.stopPropagation();
        const accountsFormType = isMnemonicDerivedGroup
            ? AccountsFormType.MnemonicSource
            : AccountsFormType.SeedSource;
        setAccountsFormValues({
            type: accountsFormType,
            sourceID: accountSource.id,
        });
        setSourceFlow(AmpliSourceFlow.ManageAccounts);
        createAccountsMutation.mutate({
            type: accountsFormType,
        });
    }

    function getUrlForLedgerDerivedAccounts(url: string) {
        const ledgerAcc = accounts.find((acc) => isLedgerAccountSerializedUI(acc));
        if (ledgerAcc) {
            const params = ledgerAcc.mainPublicKey
                ? new URLSearchParams({
                      mainPublicKey: ledgerAcc.mainPublicKey,
                  })
                : undefined;
            return url + (params ? `?${params.toString()}` : '');
        }
        return url;
    }

    function handleBalanceFinder() {
        let url = `/accounts/manage/accounts-finder/${accountSourceID}`;
        if (accountSourceID === AllowedAccountSourceTypes.LedgerDerived) {
            url = getUrlForLedgerDerivedAccounts(url);
        }
        navigate(url);
    }

    function handleExportMnemonic() {
        navigate(`../export/passphrase/${accountSource!.id}`);
    }

    function handleExportSeed() {
        navigate(`../export/seed/${accountSource!.id}`);
    }

    const featureAccountFinderEnabled = useFeature<boolean>(Feature.AccountFinder).value;

    const showBalanceFinder =
        ACCOUNTS_WITH_ENABLED_BALANCE_FINDER.includes(type) && featureAccountFinderEnabled;
    const dropdownVisibility = {
        showExportMnemonic: isMnemonicDerivedGroup && accountSource,
        showExportSeed: isSeedDerivedGroup && accountSource,
        showBalanceFinder,
    };
    const showMoreButton = Object.values(dropdownVisibility).some((v) => v);

    const hasLegacyAccount = accounts.some((account) => isLegacyAccount(account));

    function groupAccountsByAccountIndex(accounts: SerializedUIAccount[]) {
        const accountWalletGroups = accounts.reduce(
            (map, account) => {
                if (
                    isMnemonicSerializedUiAccount(account) ||
                    isSeedSerializedUiAccount(account) ||
                    isKeystoneAccountSerializedUI(account)
                ) {
                    const { accountIndex } = parseDerivationPath(account.derivationPath);
                    (map[accountIndex] ||= []).push(account);
                }
                return map;
            },
            {} as Record<number, SerializedUIAccount[]>,
        );

        return Object.fromEntries(
            Object.entries(accountWalletGroups)
                .sort(([a], [b]) => Number(a) - Number(b))
                .map(([index, accounts]) => [`Wallet ${Number(index) + 1}`, accounts]),
        );
    }

    return (
        <div className="relative overflow-visible">
            <Collapsible
                isOpen={isCollapsibleGroupOpen}
                onOpenChange={setIsCollapsibleGroupOpen}
                hideArrow
                hideBorder
                render={({ isOpen }) => (
                    <div className="relative flex min-h-[52px] w-full items-center justify-between gap-1 py-2 pl-1 pr-sm">
                        <div className="flex items-center gap-1">
                            <TriangleDown
                                className={clsx(
                                    'h-5 w-5 text-iota-neutral-10 dark:text-iota-neutral-40',
                                    isOpen
                                        ? 'rotate-0 transition-transform ease-linear'
                                        : '-rotate-90 transition-transform ease-linear',
                                )}
                            />
                            <div className="text-title-md text-iota-neutral-10 dark:text-iota-neutral-92">
                                {getGroupTitle(accounts[0])}
                            </div>
                        </div>
                        <div className="flex items-center gap-1">
                            {(isMnemonicDerivedGroup || isSeedDerivedGroup) && accountSource ? (
                                <Button
                                    size={ButtonSize.Small}
                                    type={ButtonType.Ghost}
                                    onClick={handleAdd}
                                    icon={
                                        <Add className="h-5 w-5 text-iota-neutral-10 dark:text-iota-neutral-92" />
                                    }
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
                                        icon={
                                            <MoreHoriz className="h-5 w-5 text-iota-neutral-10 dark:text-iota-neutral-92" />
                                        }
                                    />
                                </div>
                            )}
                        </div>
                    </div>
                )}
            >
                {hasLegacyAccount ? (
                    <div className="pl-md">
                        {Object.entries(groupAccountsByAccountIndex(accounts)).map(
                            ([walletName, walletAccounts], index) => (
                                <Collapsible
                                    key={index}
                                    defaultOpen
                                    hideArrow
                                    hideBorder
                                    render={({ isOpen }) => (
                                        <div className="flex w-full items-center gap-x-md p-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                            <div className="shrink-0 text-title-sm">
                                                From {walletName}
                                            </div>
                                            <Divider />
                                            <ArrowDown
                                                className={clsx(
                                                    'h-5 w-5 shrink-0',
                                                    isOpen
                                                        ? 'rotate-0 transition-transform ease-linear'
                                                        : '-rotate-90 transition-transform ease-linear',
                                                )}
                                            />
                                        </div>
                                    )}
                                >
                                    {walletAccounts.map((account, index) => (
                                        <AccountGroupItem
                                            outerRef={outerRef}
                                            isActive={activeAccount?.address === account.address}
                                            key={account.id}
                                            account={account}
                                            showDropdownOptionsBottom={
                                                isLast &&
                                                (index === walletAccounts.length - 1 ||
                                                    index === walletAccounts.length - 2)
                                            }
                                        />
                                    ))}
                                </Collapsible>
                            ),
                        )}
                    </div>
                ) : (
                    accounts.map((account, index) => (
                        <AccountGroupItem
                            outerRef={outerRef}
                            isActive={activeAccount?.address === account.address}
                            key={account.id}
                            account={account}
                            showDropdownOptionsBottom={
                                isLast &&
                                (index === accounts.length - 1 || index === accounts.length - 2)
                            }
                        />
                    ))
                )}
            </Collapsible>
            {isDropdownOpen && (
                <div className="absolute right-3 top-3 z-[100] origin-top animate-dropdown-show rounded-lg bg-iota-neutral-100 shadow-md dark:bg-iota-neutral-6">
                    <OutsideClickHandler onOutsideClick={() => setDropdownOpen(false)}>
                        <Dropdown>
                            {dropdownVisibility.showBalanceFinder && (
                                <ListItem hideBottomBorder onClick={handleBalanceFinder}>
                                    Balance Finder
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
            )}
        </div>
    );
}
