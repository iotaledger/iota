// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { useState } from 'react';
import clsx from 'clsx';
import { formatAddress } from '@iota/iota-sdk/utils';
import { ExplorerLinkType, NicknameDialog, useUnlockAccount } from '_components';
import { useNavigate } from 'react-router-dom';
import { useAccounts } from '_app/hooks/useAccounts';
import { useExplorerLink } from '_app/hooks/useExplorerLink';
import toast from 'react-hot-toast';
import { Account, Dropdown, ListItem } from '@iota/apps-ui-kit';
import { OutsideClickHandler } from '_components/OutsideClickHandler';
import { IotaLogoMark, Ledger } from '@iota/ui-icons';
import { RemoveDialog } from './RemoveDialog';
import { useBackgroundClient } from '_app/hooks/useBackgroundClient';

interface AccountGroupItemProps {
    account: SerializedUIAccount;
    isLast: boolean;
    isActive?: boolean;
}

export function AccountGroupItem({ account, isLast, isActive }: AccountGroupItemProps) {
    const [isDropdownOpen, setDropdownOpen] = useState(false);
    const [isDialogNicknameOpen, setDialogNicknameOpen] = useState(false);
    const [isDialogRemoveOpen, setDialogRemoveOpen] = useState(false);
    const accountName = account?.nickname ?? formatAddress(account?.address || '');
    const { unlockAccount, lockAccount } = useUnlockAccount();
    const navigate = useNavigate();
    const allAccounts = useAccounts();
    const backgroundClient = useBackgroundClient();

    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Address,
        address: account.address,
    });

    async function handleCopySuccess() {
        toast.success('Address copied');
    }

    function handleOpen() {
        const newWindow = window.open(explorerHref!, '_blank', 'noopener,noreferrer');
        if (newWindow) newWindow.opener = null;
    }

    function handleToggleLock(e: React.MouseEvent<HTMLButtonElement>) {
        e.stopPropagation();
        if (account.isLocked) {
            unlockAccount(account);
        } else {
            lockAccount(account);
        }
    }

    function handleRename() {
        setDialogNicknameOpen(true);
    }

    function handleExportPrivateKey() {
        navigate(`/accounts/export/${account!.id}`);
    }

    function handleRemove() {
        setDialogRemoveOpen(true);
    }

    async function handleSelectAccount() {
        if (!account) return;

        if (account.isLocked) {
            unlockAccount(account);
        } else {
            await backgroundClient.selectAccount(account.id);
            navigate('/');
            toast.success(`Account ${formatAddress(account.address)} selected`);
        }
    }

    function handleOptionsClick(e: React.MouseEvent<HTMLButtonElement>) {
        e.stopPropagation();
        setDropdownOpen(true);
    }

    return (
        <div className="relative overflow-visible [&_span]:whitespace-nowrap">
            <div onClick={handleSelectAccount}>
                <Account
                    isLocked={account.isLocked}
                    isCopyable
                    isActive={isActive}
                    copyText={account.address}
                    isExternal
                    onOpen={handleOpen}
                    avatarContent={() => <AccountAvatar account={account} />}
                    title={accountName}
                    subtitle={formatAddress(account.address)}
                    onCopy={handleCopySuccess}
                    onOptionsClick={handleOptionsClick}
                    onLockAccountClick={handleToggleLock}
                    onUnlockAccountClick={handleToggleLock}
                />
            </div>
            <div
                className={clsx(
                    `absolute right-0 z-[100] bg-white`,
                    isLast ? 'bottom-0' : 'top-0',
                    isDropdownOpen ? '' : 'hidden',
                )}
            >
                <OutsideClickHandler onOutsideClick={() => setDropdownOpen(false)}>
                    <Dropdown>
                        <ListItem hideBottomBorder onClick={handleRename}>
                            Rename
                        </ListItem>
                        {account.isKeyPairExportable ? (
                            <ListItem hideBottomBorder onClick={handleExportPrivateKey}>
                                Export Private Key
                            </ListItem>
                        ) : null}
                        {allAccounts.isPending ? null : (
                            <ListItem hideBottomBorder onClick={handleRemove}>
                                Delete
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

function AccountAvatar({ account }: { account: SerializedUIAccount }) {
    let logo = null;

    if (account.type === AccountType.LedgerDerived) {
        logo = <Ledger className="h-4 w-4" />;
    } else {
        logo = <IotaLogoMark />;
    }
    return (
        <div
            className={`flex h-8 w-8 items-center justify-center rounded-full [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-white ${account.isLocked ? 'bg-neutral-80' : 'bg-primary-30'}`}
        >
            {logo}
        </div>
    );
}
