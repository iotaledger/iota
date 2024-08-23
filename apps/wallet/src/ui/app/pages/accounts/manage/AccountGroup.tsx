// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import {
    AccountIcon,
    AccountItem,
    AccountsFormType,
    useAccountsFormContext,
    NicknameDialog,
    VerifyPasswordModal,
    ExplorerLinkType,
    useUnlockAccount,
} from '_components';
import { useResolveIotaNSName } from '@iota/core';

import { useAccounts } from '_src/ui/app/hooks/useAccounts';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useBackgroundClient } from '_src/ui/app/hooks/useBackgroundClient';
import { useCreateAccountsMutation } from '_src/ui/app/hooks/useCreateAccountMutation';
import { Button } from '_src/ui/app/shared/ButtonUI';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '_src/ui/app/shared/Dialog';
import { Button as Button2, ButtonType, ButtonSize, Account } from '@iota/apps-ui-kit';
import { Add, MoreHoriz } from '@iota/ui-icons';
import { Heading } from '_src/ui/app/shared/heading';
import { Text } from '_src/ui/app/shared/text';
import { ButtonOrLink, type ButtonOrLinkProps } from '_src/ui/app/shared/utils/ButtonOrLink';
import { ArrowBgFill16, Plus12, Search16, LedgerLogo17, Iota } from '@iota/icons';
import * as CollapsiblePrimitive from '@radix-ui/react-collapsible';
import { Collapse, CollapseBody, CollapseHeader } from './Collapse';
import { useMutation } from '@tanstack/react-query';
import { forwardRef, useState } from 'react';
import toast from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useExplorerLink } from '_app/hooks/useExplorerLink';

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

// todo: we probably have some duplication here with the various FooterLink / ButtonOrLink
// components - we should look to add these to base components somewhere
const FooterLink = forwardRef<HTMLAnchorElement | HTMLButtonElement, ButtonOrLinkProps>(
    ({ children, to, ...props }, ref) => {
        return (
            <ButtonOrLink
                ref={ref}
                className="text-hero-darkest/40 hover:text-hero-darkest/50 cursor-pointer border-none bg-transparent uppercase no-underline outline-none transition"
                to={to}
                {...props}
            >
                <Text variant="captionSmallExtra" weight="medium">
                    {children}
                </Text>
            </ButtonOrLink>
        );
    },
);

// todo: this is slightly different than the account footer in the AccountsList - look to consolidate :(
function AccountFooter({ accountID, showExport }: { accountID: string; showExport?: boolean }) {
    const allAccounts = useAccounts();
    const totalAccounts = allAccounts?.data?.length || 0;
    const backgroundClient = useBackgroundClient();
    const [isConfirmationVisible, setIsConfirmationVisible] = useState(false);
    const removeAccountMutation = useMutation({
        mutationKey: ['remove account mutation', accountID],
        mutationFn: async () => {
            await backgroundClient.removeAccount({ accountID });
            setIsConfirmationVisible(false);
        },
    });
    return (
        <>
            <div className="flex w-full flex-shrink-0">
                <div className="flex items-center gap-0.5 whitespace-nowrap">
                    <NicknameDialog
                        accountID={accountID}
                        trigger={<FooterLink>Edit Nickname</FooterLink>}
                    />
                    {showExport ? (
                        <FooterLink to={`/accounts/export/${accountID}`}>
                            Export Private Key
                        </FooterLink>
                    ) : null}
                    {allAccounts.isPending ? null : (
                        <FooterLink
                            onClick={() => setIsConfirmationVisible(true)}
                            disabled={isConfirmationVisible}
                        >
                            Remove
                        </FooterLink>
                    )}
                </div>
            </div>
            <Dialog open={isConfirmationVisible}>
                <DialogContent onPointerDownOutside={(e) => e.preventDefault()}>
                    <DialogHeader>
                        <DialogTitle>Are you sure you want to remove this account?</DialogTitle>
                    </DialogHeader>
                    {totalAccounts === 1 ? (
                        <div className="text-center">
                            <DialogDescription>
                                Removing this account will require you to set up your IOTA wallet
                                again.
                            </DialogDescription>
                        </div>
                    ) : null}
                    <DialogFooter>
                        <div className="flex gap-2.5">
                            <Button
                                variant="outline"
                                size="tall"
                                text="Cancel"
                                onClick={() => setIsConfirmationVisible(false)}
                            />
                            <Button
                                variant="warning"
                                size="tall"
                                text="Remove"
                                loading={removeAccountMutation.isPending}
                                onClick={() => {
                                    removeAccountMutation.mutate(undefined, {
                                        onSuccess: () => toast.success('Account removed'),
                                        onError: (e) =>
                                            toast.error(
                                                (e as Error)?.message || 'Something went wrong',
                                            ),
                                    });
                                }}
                            />
                        </div>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </>
    );
}

function AccountAvatar({ account }: { account: SerializedUIAccount }) {
    let logo = null;

    if (account.type === AccountType.LedgerDerived) {
        logo = <LedgerLogo17 className="h-4 w-4" />;
    } else {
        logo = <Iota />;
    }
    return (
        <div
            className={`flex h-8 w-8 items-center justify-center rounded-full [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-white ${account.isLocked ? 'bg-neutral-80' : 'bg-primary-30'}`}
        >
            {logo}
        </div>
    );
}

function AccountItem2({ account }: { account: SerializedUIAccount }) {
    const { data: domainName } = useResolveIotaNSName(account?.address);
    const accountName = account?.nickname ?? domainName ?? formatAddress(account?.address || '');
    const { unlockAccount, lockAccount } = useUnlockAccount();

    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Address,
        address: account.address,
    });

    async function handleCopy(e: React.MouseEvent<HTMLButtonElement, MouseEvent>) {
        e.preventDefault();
        await navigator.clipboard.writeText(account.address);
        toast.success('Address copied');
    }

    function handleOpen() {
        const newWindow = window.open(explorerHref!, '_blank', 'noopener,noreferrer');
        if (newWindow) newWindow.opener = null;
    }

    function handleToggleLock() {
        // prevent the account from being selected when clicking the lock button
        if (account.isLocked) {
            unlockAccount(account);
        } else {
            lockAccount(account);
        }
    }

    return (
        <Account
            isLocked={account.isLocked}
            isCopyable
            isExternal
            onOpen={handleOpen}
            avatarContent={() => <AccountAvatar account={account} />}
            title={accountName}
            subtitle={formatAddress(account.address)}
            onCopy={handleCopy}
            onOptionsClick={() => {}}
            onLockAccountClick={handleToggleLock}
            onUnlockAccountClick={handleToggleLock}
        />
    );
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

    return (
        <>
            <Collapse>
                <CollapseHeader title={getGroupTitle(accounts[0])}>
                    <div className="flex items-center gap-1">
                        {(isMnemonicDerivedGroup || isSeedDerivedGroup) && accountSource ? (
                            <Button2
                                size={ButtonSize.Small}
                                type={ButtonType.Ghost}
                                onClick={handleAdd}
                                icon={<Add className="h-5 w-5 text-neutral-10" />}
                            />
                        ) : null}
                        <Button2
                            size={ButtonSize.Small}
                            type={ButtonType.Ghost}
                            onClick={(e) => {
                                e.stopPropagation();
                                console.log('on 3 dots click.');
                            }}
                            icon={<MoreHoriz className="h-5 w-5 text-neutral-10" />}
                        />
                    </div>
                </CollapseHeader>
                <CollapseBody>
                    {accounts.map((account) => (
                        <AccountItem2 account={account} />
                    ))}
                </CollapseBody>
            </Collapse>
            <div className="">
                <CollapsiblePrimitive.Root defaultOpen asChild>
                    <div className="flex w-full flex-col gap-4">
                        <CollapsiblePrimitive.Trigger asChild>
                            <div className="group flex w-full flex-shrink-0 cursor-pointer items-center justify-center gap-2 [&>*]:select-none">
                                <ArrowBgFill16 className="text-hero-darkest/20 h-4 w-4 group-data-[state=open]:rotate-90" />
                                <Heading variant="heading5" weight="semibold" color="steel-darker">
                                    {getGroupTitle(accounts[0])}
                                </Heading>
                                <div className="bg-gray-45 flex h-px flex-1 flex-shrink-0" />
                                {ACCOUNTS_WITH_ENABLED_BALANCE_FINDER.includes(type) ? (
                                    <ButtonOrLink
                                        className="text-hero hover:text-hero-darkest flex cursor-pointer appearance-none items-center justify-center gap-0.5 border-0 bg-transparent uppercase outline-none"
                                        onClick={() => {
                                            navigate(
                                                `/accounts/manage/accounts-finder/${accountSourceID}`,
                                            );
                                        }}
                                    >
                                        <Search16 />
                                    </ButtonOrLink>
                                ) : null}
                                {(isMnemonicDerivedGroup || isSeedDerivedGroup) && accountSource ? (
                                    <>
                                        <ButtonOrLink
                                            loading={createAccountMutation.isPending}
                                            onClick={async (e) => {
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
                                            }}
                                            className="text-hero hover:text-hero-darkest flex cursor-pointer appearance-none items-center justify-center gap-0.5 border-0 bg-transparent uppercase outline-none"
                                        >
                                            <Plus12 />
                                            <Text variant="bodySmall" weight="semibold">
                                                New
                                            </Text>
                                        </ButtonOrLink>
                                    </>
                                ) : null}
                            </div>
                        </CollapsiblePrimitive.Trigger>
                        <CollapsiblePrimitive.CollapsibleContent asChild>
                            <div className="flex w-full flex-shrink-0 flex-col gap-3">
                                {accounts.map((account) => {
                                    return (
                                        <AccountItem
                                            key={account.id}
                                            background="gradient"
                                            accountID={account.id}
                                            icon={<AccountIcon account={account} />}
                                            footer={
                                                <AccountFooter
                                                    accountID={account.id}
                                                    showExport={account.isKeyPairExportable}
                                                />
                                            }
                                        />
                                    );
                                })}
                                {isMnemonicDerivedGroup && accountSource ? (
                                    <Button
                                        variant="secondary"
                                        size="tall"
                                        text="Export Passphrase"
                                        to={`../export/passphrase/${accountSource.id}`}
                                    />
                                ) : null}
                                {isSeedDerivedGroup && accountSource ? (
                                    <Button
                                        variant="secondary"
                                        size="tall"
                                        text="Export Seed"
                                        to={`../export/seed/${accountSource.id}`}
                                    />
                                ) : null}
                            </div>
                        </CollapsiblePrimitive.CollapsibleContent>
                    </div>
                </CollapsiblePrimitive.Root>
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
        </>
    );
}
