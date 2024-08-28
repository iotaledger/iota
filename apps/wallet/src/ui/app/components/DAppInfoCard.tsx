// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import { getValidDAppUrl } from '_src/shared/utils';
import { CheckFill16 } from '@iota/icons';
import cn from 'clsx';

import { useAccountByAddress } from '../hooks/useAccountByAddress';
import { AccountIcon } from './accounts/AccountIcon';
import { AccountItem } from './accounts/AccountItem';
import { LockUnlockButton } from './accounts/LockUnlockButton';
import { useUnlockAccount } from './accounts/UnlockAccountContext';
import { DAppPermissionList } from './DAppPermissionList';
import { SummaryCard } from './SummaryCard';
import { Link } from 'react-router-dom';
import { Card, CardBody, CardImage, CardType, ImageShape, ImageType } from '@iota/apps-ui-kit';

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
    const { unlockAccount, lockAccount, isPending, accountToUnlock } = useUnlockAccount();

    return (
        <div className="flex flex-col gap-y-md">
            <Card type={CardType.Default}>
                <CardImage type={ImageType.BgSolid} shape={ImageShape.Rounded} url={iconUrl} />
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
                />
            </Card>
            {connectedAddress && account ? (
                <AccountItem
                    icon={<AccountIcon account={account} />}
                    accountID={account.id}
                    disabled={account.isLocked}
                    after={
                        <div className="flex flex-1 items-center justify-end gap-1">
                            {account.isLocked ? (
                                <div className="h-4">
                                    <LockUnlockButton
                                        isLocked={account.isLocked}
                                        isLoading={isPending && accountToUnlock?.id === account.id}
                                        onClick={(e) => {
                                            // prevent the account from being selected when clicking the lock button
                                            e.stopPropagation();
                                            if (account.isLocked) {
                                                unlockAccount(account);
                                            } else {
                                                lockAccount(account);
                                            }
                                        }}
                                    />
                                </div>
                            ) : null}
                            <CheckFill16
                                className={cn(
                                    'h-4 w-4',
                                    account.isLocked ? 'text-hero/10' : 'text-success',
                                )}
                            />
                        </div>
                    }
                    hideCopy
                    hideExplorerLink
                />
            ) : null}
            {permissions?.length ? (
                <SummaryCard
                    header="Permissions requested"
                    body={<DAppPermissionList permissions={permissions} />}
                    boxShadow
                />
            ) : null}
        </div>
    );
}
