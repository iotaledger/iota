// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import { getValidDAppUrl } from '_src/shared/utils';
import { CheckFill16 } from '@iota/icons';
import cn from 'clsx';

import { useAccountByAddress } from '../hooks/useAccountByAddress';
import { Heading } from '../shared/heading';
import { Link } from '../shared/Link';
import { AccountIcon } from './accounts/AccountIcon';
import { AccountItem } from './accounts/AccountItem';
import { LockUnlockButton } from './accounts/LockUnlockButton';
import { useUnlockAccount } from './accounts/UnlockAccountContext';
import { DAppPermissionsList } from './DAppPermissionsList';
import { SummaryCard } from './SummaryCard';

export type DAppInfoCardProps = {
    name: string;
    url: string;
    iconUrl?: string;
    connectedAddress?: string;
    permissions?: PermissionType[];
};

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
    const { unlockAccount, lockAccount, isPending, accountToUnlock } = useUnlockAccount();

    return (
        <div className="flex flex-col gap-5 bg-white p-6">
            <div className="flex flex-row flex-nowrap items-center gap-3.75 py-3">
                <div className="flex h-15 w-15 shrink-0 grow-0 items-stretch overflow-hidden rounded-2xl bg-steel/20">
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
                    body={<DAppPermissionsList permissions={permissions} />}
                    boxShadow
                />
            ) : null}
        </div>
    );
}
