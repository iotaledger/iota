// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import {
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
import {
    Button as Button2,
    ButtonType,
    ButtonSize,
    Account,
    Dropdown,
    ListItem,
    Dialog,
    DialogBody,
    DialogContent,
    Header,
} from '@iota/apps-ui-kit';
import { Add, MoreHoriz } from '@iota/ui-icons';
import { LedgerLogo17, Iota } from '@iota/icons';
import { Collapse, CollapseBody, CollapseHeader } from './Collapse';
import { useMutation } from '@tanstack/react-query';
import { useState, useRef, useCallback, useEffect } from 'react';
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

function RemoveDialog({
    isOpen,
    setOpen,
    accountID,
}: {
    accountID: string;
    isOpen: boolean;
    setOpen: React.Dispatch<React.SetStateAction<boolean>>;
}) {
    const allAccounts = useAccounts();
    const totalAccounts = allAccounts?.data?.length || 0;
    const backgroundClient = useBackgroundClient();
    const removeAccountMutation = useMutation({
        mutationKey: ['remove account mutation', accountID],
        mutationFn: async () => {
            await backgroundClient.removeAccount({ accountID: accountID });
            setOpen(false);
        },
    });
    return (
        <Dialog open={isOpen} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header
                    title="Are you sure you want to remove this account?"
                    onClose={() => setOpen(false)}
                />
                <DialogBody>
                    {totalAccounts === 1 ? (
                        <div className="text-center">
                            Removing this account will require you to set up your IOTA wallet again.
                        </div>
                    ) : null}
                    <div className="flex gap-2.5">
                        <Button2
                            fullWidth
                            type={ButtonType.Secondary}
                            text="Cancel"
                            onClick={() => setOpen(false)}
                        />
                        <Button2
                            fullWidth
                            type={ButtonType.Primary}
                            text="Remove"
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
                </DialogBody>
            </DialogContent>
        </Dialog>
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

function AccountItem2({ account, isLast }: { account: SerializedUIAccount; isLast: boolean }) {
    const [isDropdownOpen, setDropdownOpen] = useState(false);
    const [isDialogNicknameOpen, setDialogNicknameOpen] = useState(false);
    const [isDialogRemoveOpen, setDialogRemoveOpen] = useState(false);
    const { data: domainName } = useResolveIotaNSName(account?.address);
    const accountName = account?.nickname ?? domainName ?? formatAddress(account?.address || '');
    const { unlockAccount, lockAccount } = useUnlockAccount();
    const navigate = useNavigate();
    const allAccounts = useAccounts();

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

    function handleEditNickname() {
        setDialogNicknameOpen(true);
    }

    function handleExportPrivateKey() {
        navigate(`/accounts/export/${account!.id}`);
    }

    function handleRemove() {
        setDialogRemoveOpen(true);
    }

    return (
        <div className="relative overflow-visible">
            <Account
                isLocked={account.isLocked}
                isCopyable
                isExternal
                onOpen={handleOpen}
                avatarContent={() => <AccountAvatar account={account} />}
                title={accountName}
                subtitle={formatAddress(account.address)}
                onCopy={handleCopy}
                onOptionsClick={() => setDropdownOpen(true)}
                onLockAccountClick={handleToggleLock}
                onUnlockAccountClick={handleToggleLock}
            />
            <div
                className={`absolute right-0 ${isLast ? 'bottom-0' : 'top-0'} z-[100] bg-white ${isDropdownOpen ? '' : 'hidden'}`}
            >
                <OutsideClickHandler onOutsideClick={() => setDropdownOpen(false)}>
                    <Dropdown>
                        <ListItem hideBottomBorder onClick={handleEditNickname}>
                            Edit Nickname
                        </ListItem>
                        {account.isKeyPairExportable ? (
                            <ListItem hideBottomBorder onClick={handleExportPrivateKey}>
                                Export Private Key
                            </ListItem>
                        ) : null}
                        {allAccounts.isPending ? null : (
                            <ListItem hideBottomBorder onClick={handleRemove}>
                                Remove
                            </ListItem>
                        )}
                    </Dropdown>
                </OutsideClickHandler>
            </div>
            <NicknameDialog
                isOpen={isDialogNicknameOpen}
                setOpen={setDialogNicknameOpen}
                accountID={account.id}
            />
            <RemoveDialog
                isOpen={isDialogRemoveOpen}
                setOpen={setDialogRemoveOpen}
                accountID={account.id}
            />
        </div>
    );
}

interface OutsideClickHandlerProps {
    onOutsideClick: () => void;
    children: React.ReactNode;
}

const OutsideClickHandler: React.FC<OutsideClickHandlerProps> = ({ onOutsideClick, children }) => {
    const ref = useRef<HTMLDivElement>(null);

    const handleClick = useCallback(
        (event: MouseEvent) => {
            if (ref.current && !ref.current.contains(event.target as Node)) {
                onOutsideClick();
            }
        },
        [onOutsideClick],
    );

    useEffect(() => {
        document.addEventListener('mousedown', handleClick);
        return () => {
            document.removeEventListener('mousedown', handleClick);
        };
    }, [handleClick]);

    return <div ref={ref}>{children}</div>;
};

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
                            <Button2
                                size={ButtonSize.Small}
                                type={ButtonType.Ghost}
                                onClick={handleAdd}
                                icon={<Add className="h-5 w-5 text-neutral-10" />}
                            />
                        ) : null}
                        <div className="relative">
                            <Button2
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
                        <AccountItem2 account={account} isLast={index === accounts.length - 1} />
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
