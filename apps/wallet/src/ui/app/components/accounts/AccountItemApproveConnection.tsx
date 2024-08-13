// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountIcon, LockUnlockButton, useUnlockAccount } from '_components';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { useResolveIotaNSName } from '@iota/core';
import { CheckFill16 } from '@iota/icons';
import { formatAddress } from '@iota/iota-sdk/utils';
import clsx from 'clsx';

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
    const { unlockAccount, lockAccount, isPending, accountToUnlock } = useUnlockAccount();

    return (
        <div
            className={clsx(
                'border-hero/10 group cursor-pointer rounded-xl border border-solid px-4 py-3',
                'flex items-center justify-start gap-3',
                selected ? 'bg-white/80 shadow-card-soft' : 'bg-white/40 hover:bg-white/60',
                disabled ? '!bg-hero-darkest/10 border-transparent' : 'hover:shadow',
            )}
        >
            <AccountIcon account={account} />

            <div className="flex flex-col items-start gap-1 overflow-hidden">
                <div
                    className={clsx(
                        'group-hover:text-steel-darker truncate font-sans text-body font-semibold',
                        selected ? 'text-steel-darker' : 'text-steel-dark',
                        disabled && '!text-steel-darker',
                    )}
                >
                    {accountName}
                </div>

                <div
                    className={clsx(
                        'truncate font-mono text-subtitle font-semibold',
                        disabled ? 'text-steel-darker' : 'text-steel group-hover:text-steel-dark',
                    )}
                >
                    {formatAddress(account.address)}
                </div>
            </div>

            <div className="ml-auto flex gap-4">
                {showLock ? (
                    <div className="flex items-center justify-center">
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

                <div
                    className={clsx(`text-hero/10 ml-auto flex items-center justify-center`, {
                        'text-success': selected,
                    })}
                >
                    <CheckFill16 className={clsx('h-4 w-4', { 'opacity-50': !selected })} />
                </div>
            </div>
        </div>
    );
}
