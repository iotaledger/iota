// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import { getValidDAppUrl } from '_src/shared/utils';
import { Card, CardBody, CardImage, CardType, ImageShape, ImageType } from '@iota/apps-ui-kit';
import { useAccountByAddress } from '_hooks';
import { AccountIcon } from './accounts/AccountIcon';
import { AccountItem } from './accounts/AccountItem';
import { useUnlockAccount } from './accounts/UnlockAccountContext';
import { DAppPermissionList } from './DAppPermissionList';
import { SummaryCard } from './SummaryCard';
import { Link } from 'react-router-dom';
import { ImageIcon } from '@iota/core';

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
        <div className="flex flex-col gap-y-md">
            <Card type={CardType.Default}>
                <CardImage type={ImageType.BgSolid} shape={ImageShape.Rounded}>
                    <ImageIcon src={iconUrl || null} label={name} fallback={name} />
                </CardImage>
                <CardBody
                    title={name}
                    subtitle={
                        <Link
                            to={validDAppUrl?.toString() ?? url}
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            {validDAppUrl?.toString() ?? url}
                        </Link>
                    }
                    isTextTruncated
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
                    body={<DAppPermissionList permissions={permissions} />}
                />
            ) : null}
        </div>
    );
}
