// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { useState } from 'react';
import { useResolveIotaNSName } from '@iota/core';
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

export function AccountGroupItem({
    account,
    isLast,
}: {
    account: SerializedUIAccount;
    isLast: boolean;
}) {
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

    function handleDelete() {
        setDialogRemoveOpen(true);
    }

    return (
        <div className="relative overflow-visible [&_span]:whitespace-nowrap">
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
                        <ListItem hideBottomBorder onClick={handleRename}>
                            Rename
                        </ListItem>
                        {account.isKeyPairExportable ? (
                            <ListItem hideBottomBorder onClick={handleExportPrivateKey}>
                                Export Private Key
                            </ListItem>
                        ) : null}
                        {allAccounts.isPending ? null : (
                            <ListItem hideBottomBorder onClick={handleDelete}>
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
