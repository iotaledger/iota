// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountIcon } from '_components/accounts/AccountIcon';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { CheckFill16 } from '@iota/icons';
import { formatAddress } from '@iota/iota.js/utils';
import clsx from 'clsx';

interface AccountsFinderItemProps {
    account: SerializedUIAccount;
    disabled: boolean;
    selected?: boolean;
    formattedBalance: string;
}

export function AccountsFinderItem({
    account,
    selected,
    disabled,
    formattedBalance,
}: AccountsFinderItemProps) {
    const accountName = account?.nickname ?? formatAddress(account?.address || '');

    return (
        <div
            className={clsx(
                'group cursor-pointer rounded-xl border border-solid border-hero/10 px-4 py-3',
                'flex items-center justify-start gap-3',
                selected ? 'bg-white/80 shadow-card-soft' : 'bg-white/40 hover:bg-white/60',
                disabled ? 'border-transparent !bg-hero-darkest/10' : 'hover:shadow',
            )}
        >
            <AccountIcon account={account} />

            <div className="flex flex-col items-start gap-1 overflow-hidden">
                <div
                    className={clsx(
                        'truncate font-sans text-body font-semibold group-hover:text-steel-darker',
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
                <div
                    className={clsx(
                        'font-mono text-subtitle font-semibold',
                        disabled ? 'text-steel-darker' : 'text-steel group-hover:text-steel-dark',
                    )}
                >
                    {formattedBalance}
                </div>
            </div>

            <div className="ml-auto flex gap-4">
                <div
                    className={clsx(`ml-auto flex items-center justify-center text-hero/10`, {
                        'text-success': selected,
                    })}
                >
                    <CheckFill16 className={clsx('h-4 w-4', { 'opacity-50': !selected })} />
                </div>
            </div>
        </div>
    );
}
