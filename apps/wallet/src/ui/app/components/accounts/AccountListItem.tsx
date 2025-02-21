// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { AccountItem } from './AccountItem';
import { IotaLogoMark } from '@iota/apps-ui-icons';

interface AccountListItemProps {
    account: SerializedUIAccount;
    editable?: boolean;
    showLock?: boolean;
    hideCopy?: boolean;
    hideExplorerLink?: boolean;
    onLockAccountClick?: () => void;
    onUnlockAccountClick?: () => void;
    icon?: React.ReactNode;
}

export function AccountListItem({
    account,
    hideCopy,
    hideExplorerLink,
    onLockAccountClick,
    onUnlockAccountClick,
    icon,
}: AccountListItemProps) {
    return (
        <AccountItem
            icon={icon ?? <IotaLogoMark />}
            onLockAccountClick={onLockAccountClick}
            onUnlockAccountClick={onUnlockAccountClick}
            accountID={account.id}
            hideCopy={hideCopy}
            hideExplorerLink={hideExplorerLink}
        />
    );
}
