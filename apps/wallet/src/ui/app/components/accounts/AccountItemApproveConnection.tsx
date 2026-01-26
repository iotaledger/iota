// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountIcon, useUnlockAccounts } from '_components';
import { type SerializedUIAccount } from '_src/background/accounts/account';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Account } from '@iota/apps-ui-kit';
import { formatAccountName } from '../../helpers';
import { useGetDefaultIotaName } from '@iota/core';

interface AccountItemApproveConnectionProps {
    account: SerializedUIAccount;
    selected?: boolean;
    onLock?: (id: string) => void;
}

export function AccountItemApproveConnection({
    account,
    selected,
    onLock,
}: AccountItemApproveConnectionProps) {
    const { data: iotaName } = useGetDefaultIotaName(account?.address);
    const accountName = formatAccountName(account?.nickname, iotaName, account?.address);

    const { unlockAccounts, lockAccounts } = useUnlockAccounts();

    function onUnlockedAccountClick() {
        if (account.isLocked && account.isPasswordUnlockable) {
            unlockAccounts();
        }
    }

    return (
        <div onClick={onUnlockedAccountClick}>
            <Account
                title={accountName}
                subtitle={formatAddress(account.address)}
                isSelected={selected}
                isLocked={account.isLocked}
                showSelected={true}
                onLockAccountClick={(event) => {
                    event.stopPropagation();
                    lockAccounts();
                    onLock?.(account.id);
                }}
                onUnlockAccountClick={(event) => {
                    event.stopPropagation();
                    unlockAccounts();
                }}
                avatarContent={() => <AccountIcon account={account} />}
            />
        </div>
    );
}
