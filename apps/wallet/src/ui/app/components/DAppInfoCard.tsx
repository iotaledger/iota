// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import { getValidDAppUrl } from '_src/shared/utils';
import { useAccountByAddress } from '../hooks/useAccountByAddress';
import { Heading } from '../shared/heading';
import { Link } from '../shared/Link';
import { AccountIcon } from './accounts/AccountIcon';
import { AccountItem } from './accounts/AccountItem';
import { useUnlockAccount } from './accounts/UnlockAccountContext';
import { DAppPermissionsList } from './DAppPermissionsList';
import { SummaryCard } from './SummaryCard';

export interface DAppInfoCardProps {
    name: string;
    url: string;
    iconUrl?: string;
    connectedAddress?: string;
    permissions?: PermissionType[];
}

export function DAppInfoCard({
    name,
    url,
    iconUrl,
    connectedAddress,
    permissions,
}: DAppInfoCardProps) {
    const validDAppUrl = getValidDAppUrl(url);
    const appHostname = validDAppUrl?.hostname ?? url;
    const { data: account } = useAccountByAddress(connectedAddress);
    const { unlockAccount, lockAccount } = useUnlockAccount();
    function handleLockAndUnlockClick() {
        if (!account) return;
        if (account?.isLocked) {
            unlockAccount(account);
        } else {
            lockAccount(account);
        }
    }
    return (
        <div className="flex flex-col gap-5 bg-white p-6">
            <div className="flex flex-row flex-nowrap items-center gap-3.75 py-3">
                <div className="bg-steel/20 flex h-15 w-15 shrink-0 grow-0 items-stretch overflow-hidden rounded-2xl">
                    {iconUrl ? <img className="flex-1" src={iconUrl} alt={name} /> : null}
                </div>
                <div className="flex flex-col flex-nowrap items-start gap-1 overflow-hidden">
                    <div className="max-w-full overflow-hidden">
                        <Heading variant="heading4" weight="semibold" color="gray-100" truncate>
                            {name}
                        </Heading>
                    </div>
                    <div className="max-w-full overflow-hidden">
                        <Link
                            href={validDAppUrl?.toString() ?? url}
                            title={name}
                            text={appHostname}
                            color="heroDark"
                            weight="medium"
                        />
                    </div>
                </div>
            </div>
            {connectedAddress && account ? (
                <AccountItem
                    icon={<AccountIcon account={account} />}
                    accountID={account.id}
                    onLockAccountClick={handleLockAndUnlockClick}
                    onUnlockAccountClick={handleLockAndUnlockClick}
                    hideCopy
                    hideExplorerLink
                />
            ) : null}
            {permissions?.length ? (
                <SummaryCard
                    header="Permissions requested"
                    body={<DAppPermissionsList permissions={permissions} />}
                    boxShadow
                />
            ) : null}
        </div>
    );
}
