// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import { getValidDAppUrl } from '_src/shared/utils';
import { Card, CardBody, CardImage } from '@iota/apps-ui-kit';
import { useAccountByAddress } from '../hooks/useAccountByAddress';
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
        <div className="flex flex-col gap-md bg-white p-md">
            <Card>
                <CardImage>
                    {iconUrl ? <img className="flex-1" src={iconUrl} alt={name} /> : null}
                </CardImage>
                <CardBody
                    title={name}
                    subtitle={
                        <a
                            target="_blank"
                            rel="noreferrer noopener"
                            href={validDAppUrl?.toString() ?? url}
                        >
                            {appHostname}
                        </a>
                    }
                />
            </Card>
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
