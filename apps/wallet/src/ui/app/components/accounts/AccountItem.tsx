// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useResolveIotaNSName } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import cn from 'clsx';
import { forwardRef, type ReactNode } from 'react';
import { useAccounts } from '../../hooks/useAccounts';
import { useCopyToClipboard } from '../../hooks/useCopyToClipboard';
import { useExplorerLink } from '../../hooks/useExplorerLink';
import { ExplorerLinkType } from '_components';
import { Account } from '@iota/apps-ui-kit';

interface AccountItemProps {
    accountID: string;
    icon?: ReactNode;
    hideExplorerLink?: boolean;
    hideCopy?: boolean;
    onLockAccountClick?: () => void;
    onUnlockAccountClick?: () => void;
}

export const AccountItem = forwardRef<HTMLDivElement, AccountItemProps>(
    ({
        icon,
        accountID,
        onLockAccountClick,
        onUnlockAccountClick,
        hideExplorerLink,
        hideCopy,
    }: AccountItemProps) => {
        const { data: accounts } = useAccounts();
        const account = accounts?.find((account) => account.id === accountID);
        const { data: domainName } = useResolveIotaNSName(account?.address);
        const accountName =
            account?.nickname ?? domainName ?? formatAddress(account?.address || '');
        const copyAddress = useCopyToClipboard(account?.address || '', {
            copySuccessMessage: 'Address copied',
        });
        const explorerHref = useExplorerLink({
            type: ExplorerLinkType.Address,
            address: account?.address,
        });
        if (!account) return null;

        function handleOpen() {
            const newWindow = window.open(explorerHref!, '_blank', 'noopener,noreferrer');
            if (newWindow) newWindow.opener = null;
        }
        return (
            <Account
                title={accountName}
                subtitle={formatAddress(account.address)}
                isLocked={account.isLocked}
                onOpen={handleOpen}
                avatarContent={() => <AccountAvatar isLocked={account.isLocked} icon={icon} />}
                onCopy={copyAddress}
                isCopyable={!hideCopy}
                isExternal={!hideExplorerLink}
                onLockAccountClick={onLockAccountClick}
                onUnlockAccountClick={onUnlockAccountClick}
            />
        );
    },
);

function AccountAvatar({ isLocked, icon }: { isLocked?: boolean; icon?: ReactNode }) {
    return (
        <div
            className={cn(
                'flex h-10 w-10 items-center justify-center rounded-full [&_svg]:h-5 [&_svg]:w-5 ',
                isLocked
                    ? 'bg-neutral-96 [&_svg]:text-neutral-10'
                    : 'bg-primary-30 [&_svg]:text-white',
            )}
        >
            {icon}
        </div>
    );
}
