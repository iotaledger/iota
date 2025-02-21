// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountIcon, useUnlockAccount } from '_components';
import { type SerializedUIAccount } from '_src/background/accounts/account';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Account } from '@iota/apps-ui-kit';
import { formatAccountName } from '../../helpers';

interface AccountItemApproveConnectionProps {
    account: SerializedUIAccount;
    selected?: boolean;
}

export function AccountItemApproveConnection({
    account,
    selected,
}: AccountItemApproveConnectionProps) {
    const accountName = formatAccountName(account?.nickname, account?.address);

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
                onLockAccountClick={() => lockAccount(account)}
                onUnlockAccountClick={() => unlockAccount(account)}
                avatarContent={() => <AccountIcon account={account} />}
            />
        </div>
    );
}
