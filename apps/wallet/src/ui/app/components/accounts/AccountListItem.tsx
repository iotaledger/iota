// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { AccountItem } from './AccountItem';
import { Key } from '@iota/ui-icons';

interface AccountListItemProps {
    account: SerializedUIAccount;
    editable?: boolean;
    showLock?: boolean;
    hideCopy?: boolean;
    hideExplorerLink?: boolean;
    onLockAccountClick?: () => void;
    onUnlockAccountClick?: () => void;
}

export function AccountListItem({
    account,
    hideCopy,
    hideExplorerLink,
    onLockAccountClick,
    onUnlockAccountClick,
}: AccountListItemProps) {
    return (
        <AccountItem
            icon={<Key />}
            onLockAccountClick={onLockAccountClick}
            onUnlockAccountClick={onUnlockAccountClick}
            accountID={account.id}
            hideCopy={hideCopy}
            hideExplorerLink={hideExplorerLink}
        />
    );
}
