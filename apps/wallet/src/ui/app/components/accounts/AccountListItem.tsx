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
    icon?: React.ReactNode;
}

export function AccountListItem({
    account,
    hideCopy,
    hideExplorerLink,
    icon,
}: AccountListItemProps) {
    return (
        <AccountItem
            icon={icon ?? <IotaLogoMark />}
            accountID={account.id}
            hideCopy={hideCopy}
            hideExplorerLink={hideExplorerLink}
        />
    );
}
