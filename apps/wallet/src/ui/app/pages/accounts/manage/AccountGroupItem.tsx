// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { useState, useRef } from 'react';
import clsx from 'clsx';
import { formatAddress } from '@iota/iota-sdk/utils';
import { ExplorerLinkType, NicknameDialog, useUnlockAccount } from '_components';
import { useNavigate } from 'react-router-dom';
import { useAccounts, useExplorerLink, useBackgroundClient } from '_hooks';
import { toast } from '@iota/core';
import { Account, BadgeType, Dropdown, ListItem } from '@iota/apps-ui-kit';
import { OutsideClickHandler } from '_components/OutsideClickHandler';
import { IotaLogoMark, Ledger } from '@iota/apps-ui-icons';
import { RemoveDialog } from './RemoveDialog';
import { isMainAccount } from '_src/background/accounts/isMainAccount';
import { Portal } from '_app/shared/Portal';
import { formatAccountName } from '_src/ui/app/helpers';
import { isLegacyAccount } from '_src/background/accounts/isLegacyAccount';

interface AccountGroupItemProps {
    account: SerializedUIAccount;
    showDropdownOptionsBottom: boolean;
    isActive?: boolean;
    outerRef?: React.RefObject<HTMLDivElement>;
}

export function AccountGroupItem({
    account,
    showDropdownOptionsBottom,
    isActive,
    outerRef,
}: AccountGroupItemProps) {
    const [isDropdownOpen, setDropdownOpen] = useState(false);
    const [dropdownPosition, setDropdownPosition] = useState({
        y: 0,
    });
    const anchorRef = useRef<HTMLDivElement>(null);
    const [isDialogNicknameOpen, setDialogNicknameOpen] = useState(false);
    const [isDialogRemoveOpen, setDialogRemoveOpen] = useState(false);
    const { unlockAccount, lockAccount } = useUnlockAccount();
    const navigate = useNavigate();
    const allAccounts = useAccounts();
    const backgroundClient = useBackgroundClient();
    const accountName = formatAccountName(account?.nickname, account?.address);

    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Address,
        address: account.address,
    });

    async function handleCopySuccess() {
        toast('Address copied');
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

        await backgroundClient.selectAccount(account.id);
        navigate('/');
        toast(`Account ${formatAddress(account.address)} selected`);
    }

    function handleOptionsClick(e: React.MouseEvent<HTMLButtonElement>) {
        const outerTop = outerRef?.current?.getBoundingClientRect().top;
        const innerTop = anchorRef?.current?.getBoundingClientRect().top;
        const innerHeight = anchorRef?.current?.getBoundingClientRect().height;
        e.stopPropagation();

        let y = 0;

        if (innerTop && outerTop) {
            y = innerTop - outerTop;
        }

        if (showDropdownOptionsBottom && innerHeight) {
            y = y + innerHeight;
        }

        setDropdownPosition({
            y: y,
        });
        setDropdownOpen(true);
    }

    const isMain = isMainAccount(account);
    const isLegacy = isLegacyAccount(account);

    const badgeConfig = isMain
        ? {
              type: BadgeType.PrimarySoft,
              text: 'Main',
          }
        : isLegacy
          ? {
                type: BadgeType.Neutral,
                text: 'Legacy',
            }
          : {
                type: undefined,
                text: undefined,
            };
    return (
        <div className="relative overflow-visible [&_span]:whitespace-nowrap">
            <div onClick={handleSelectAccount} ref={anchorRef}>
                <Account
                    isLocked={account.isLocked}
                    isCopyable
                    isActive={isActive}
                    copyText={account.address}
                    isExternal
                    onOpen={handleOpen}
                    avatarContent={() => <AccountAvatar account={account} />}
                    title={accountName}
                    badgeType={badgeConfig.type}
                    badgeText={badgeConfig.text}
                    subtitle={formatAddress(account.address)}
                    onCopy={handleCopySuccess}
                    onOptionsClick={handleOptionsClick}
                    onLockAccountClick={handleToggleLock}
                    onUnlockAccountClick={handleToggleLock}
                    badgeTooltipText={
                        isLegacy
                            ? 'Legacy address from the Chrysalis era. May not be supported by newer wallets, please consider migrating funds'
                            : undefined
                    }
                />
            </div>
            <Portal containerId={'manage-account-item-portal-container'}>
                {isDropdownOpen && (
                    <div
                        style={{
                            top: dropdownPosition.y,
                        }}
                        className={clsx(
                            `absolute right-0 z-[99] rounded-lg bg-neutral-100 shadow-md dark:bg-neutral-6`,
                            showDropdownOptionsBottom ? '-translate-y-full' : '',
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
                )}
            </Portal>
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
            className={`flex h-8 w-8 items-center justify-center rounded-full [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-neutral-100 ${account.isLocked ? 'bg-neutral-90 dark:bg-neutral-20 [&_svg]:dark:text-neutral-50' : 'bg-primary-30 '}`}
        >
            {logo}
        </div>
    );
}
