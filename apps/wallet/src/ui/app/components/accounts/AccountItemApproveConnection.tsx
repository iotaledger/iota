// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountIcon, useUnlockAccount } from '_components';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { Account } from '@iota/apps-ui-kit';
import { useResolveIotaNSName } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';

interface AccountItemApproveConnectionProps {
    account: SerializedUIAccount;
    disabled: boolean;
    selected?: boolean;
    showLock?: boolean;
}

export function AccountItemApproveConnection({
    account,
    selected,
    disabled,
    showLock,
}: AccountItemApproveConnectionProps) {
    const { data: domainName } = useResolveIotaNSName(account?.address);
    const accountName = account?.nickname ?? domainName ?? formatAddress(account?.address || '');
    const { unlockAccount, lockAccount } = useUnlockAccount();

    return (
        <>
            <Account
                title={accountName}
                subtitle={formatAddress(account.address)}
                isSelected={selected}
                isLocked={account.isLocked}
                onLockAccountClick={() => lockAccount(account)}
                onUnlockAccountClick={() => unlockAccount(account)}
                avatarContent={() => <AccountIcon account={account} />}
            />
        </>
    );
}
