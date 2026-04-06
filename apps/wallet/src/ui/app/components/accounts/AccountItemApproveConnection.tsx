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
}

export function AccountItemApproveConnection({
    account,
    selected,
}: AccountItemApproveConnectionProps) {
    const { data: iotaName } = useGetDefaultIotaName(account?.address);
    const accountName = formatAccountName(account?.nickname, iotaName, account?.address);

    const { unlockAccounts } = useUnlockAccounts();

    function onUnlockedAccountClick() {
        if (account.isLocked && account.isPasswordUnlockable) {
            unlockAccounts();
        }
    }

    return (
        <div onClick={onUnlockedAccountClick} data-amp-mask>
            <Account
                title={accountName}
                subtitle={formatAddress(account.address)}
                isSelected={selected}
                showSelected={true}
                avatarContent={() => <AccountIcon account={account} />}
            />
        </div>
    );
}
