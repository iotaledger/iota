// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cx } from 'class-variance-authority';
import { useAccounts } from '_hooks';
import { useMemo } from 'react';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Checkbox } from '@iota/apps-ui-kit';

export interface WalletListSelectProps {
    values: string[];
    visibleValues?: string[];
    mode?: WalletListSelectMode;
    disabled?: boolean;
    onChange: (values: string[]) => void;
    boxShadow?: boolean;
}

enum WalletListSelectMode {
    Select = 'select',
    Disconnect = 'disconnect',
}

export function WalletListSelect({
    values,
    visibleValues,
    disabled,
    onChange,
}: WalletListSelectProps) {
    const { data: accounts } = useAccounts();

    const filteredAccounts = useMemo(() => {
        if (!accounts) {
            return [];
        }
        if (visibleValues) {
            return accounts.filter(({ address }) => visibleValues.includes(address));
        }
        return accounts;
    }, [accounts, visibleValues]);

    function onAccountClick(address: string) {
        if (disabled) {
            return;
        }
        const newValues = [];
        let found = false;
        for (const anAddress of values) {
            if (anAddress === address) {
                found = true;
                continue;
            }
            newValues.push(anAddress);
        }
        if (!found) {
            newValues.push(address);
        }
        onChange(newValues);
    }

    return (
        <div className="flex flex-col gap-y-sm">
            {filteredAccounts.map(({ address }) => {
                const accountAddress = formatAddress(address);
                return (
                    <div
                        key={address}
                        className="flex cursor-default flex-row items-center justify-start gap-x-xs py-xxxs"
                        onClick={() => onAccountClick(address)}
                    >
                        <Checkbox name={address} isChecked={values.includes(address)} />
                        <span
                            className={cx(
                                'cursor-default text-body-md text-neutral-40',
                                disabled && 'text-opacity-40',
                            )}
                        >
                            {accountAddress}
                        </span>
                    </div>
                );
            })}
        </div>
    );
}
