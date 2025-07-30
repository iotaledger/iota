// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountIcon, useUnlockAccount } from '_components';
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
    const { data: iotaName } = useGetDefaultIotaName(account?.address, true);
    const accountName = formatAccountName(account?.nickname, iotaName, account?.address);

    const { unlockAccount, lockAccount } = useUnlockAccount();

    function onUnlockedAccountClick() {
        if (account.isLocked) {
            unlockAccount(account);
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
                    lockAccount(account);
                    onLock?.(account.id);
                }}
                onUnlockAccountClick={(event) => {
                    event.stopPropagation();
                    unlockAccount(account);
                }}
                avatarContent={() => <AccountIcon account={account} />}
            />
        </div>
    );
}
