// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowDown } from '@iota/ui-icons';
import classNames from 'classnames';

enum AccountSwitcherType {
    AccountSwitcher,
    Connected,
}

interface AccountSwitcherProps<T> {
    accounts: T[];
    text?: string;
    onAccountChange: (account: T) => void;
    type: AccountSwitcherType;
    isOpen?: boolean;
    onClick?: (e: React.MouseEvent<HTMLDivElement>) => void;
}

const ACCOUNT_SWITCHER_STYLE = {
    [AccountSwitcherType.AccountSwitcher]: 'border-neutral-80 bg-transparent',
    [AccountSwitcherType.Connected]: 'border-neutral-90 bg-neutral-90',
};

export function AccountSwitcher<T>({
    accounts,
    text,
    onAccountChange,
    type = AccountSwitcherType.AccountSwitcher,
    isOpen,
    onClick,
}: AccountSwitcherProps<T>) {
    return (
        <div
            className={classNames(
                'relative flex cursor-pointer items-center justify-center gap-xs rounded-full border px-md py-xs text-neutral-10 dark:text-neutral-92',
                'state-layer',
                ACCOUNT_SWITCHER_STYLE[type],
            )}
            onClick={onClick}
        >
            <span>{text}</span>
            <ArrowDown
                className={classNames('h-5 w-5 transition-transform', { 'rotate-180': isOpen })}
            />
        </div>
    );
}
