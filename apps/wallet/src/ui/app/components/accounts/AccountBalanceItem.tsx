// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { Collapsible } from '@iota/core';
import { TriangleDown } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { Badge, BadgeType } from '@iota/apps-ui-kit';

interface AccountBalanceItemProps {
    account: SerializedUIAccount;
}

export function AccountBalanceItem({ account }: AccountBalanceItemProps): JSX.Element {
    // Replace the following mocked data with the actual data
    const MOCKED_DATA = {
        walletName: 'Wallet X',
        totalBalance: '10.99 IOTA',
        addresses: [
            {
                address: '0x5e93…5928',
                balance: '1.99 IOTA',
            },
            {
                address: '0x4d93…3679',
                balance: '2.99 IOTA',
            },
            {
                address: '0x510…8898',
                balance: '3.99 IOTA',
            },
            {
                address: '0x5e20…5990',
                balance: '2.99 IOTA',
            },
        ],
        pills: ['Assets', 'Legacy', 'Vesting'],
    };
    return (
        <Collapsible
            defaultOpen
            hideArrow
            render={({ isOpen }) => (
                <div className="relative flex min-h-[52px] w-full items-center justify-between gap-1 py-2 pl-1 pr-sm text-neutral-10 dark:text-neutral-92">
                    <div className="flex items-center gap-xxs">
                        <TriangleDown
                            className={clsx(
                                'h-5 w-5 ',
                                isOpen
                                    ? 'rotate-0 transition-transform ease-linear'
                                    : '-rotate-90 transition-transform ease-linear',
                            )}
                        />
                        <div className="flex flex-col items-start gap-xxs">
                            <div className="text-title-md">{MOCKED_DATA.walletName}</div>
                            <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                                {MOCKED_DATA.addresses.length} addresses
                            </span>
                        </div>
                    </div>
                    <div className="flex flex-col items-end gap-xxs">
                        <span>{MOCKED_DATA.totalBalance}</span>
                        <div className="flex flex-row gap-xxs">
                            {MOCKED_DATA.pills.map((pill) => (
                                <Badge type={BadgeType.Neutral} label={pill} />
                            ))}
                        </div>
                    </div>
                </div>
            )}
        >
            <div className="flex flex-col gap-y-sm p-sm pl-lg text-body-md text-neutral-10 dark:text-neutral-92">
                {MOCKED_DATA.addresses.map(({ address, balance }) => (
                    <div className="flex w-full flex-row justify-between">
                        <span>{address}</span>
                        <span>{balance}</span>
                    </div>
                ))}
            </div>
        </Collapsible>
    );
}
