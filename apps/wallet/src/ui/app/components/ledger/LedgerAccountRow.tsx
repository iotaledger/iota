// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';
import { useBalance, useFormatCoin, useResolveIotaNSName } from '@iota/core';
import { CheckFill16 } from '@iota/icons';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import cl from 'clsx';

interface LedgerAccountRowProps {
    isSelected: boolean;
    address: string;
}

export function LedgerAccountRow({ isSelected, address }: LedgerAccountRowProps) {
    const { data: coinBalance } = useBalance(address);
    const { data: domainName } = useResolveIotaNSName(address);
    const [totalAmount, totalAmountSymbol] = useFormatCoin(
        coinBalance?.totalBalance ?? 0,
        IOTA_TYPE_ARG,
    );

    return (
        <div className="flex items-center gap-3">
            <CheckFill16
                className={cl('h-4 w-4', {
                    'text-gray-50': !isSelected,
                    'text-success': isSelected,
                })}
            />
            <Text
                mono
                variant="bodySmall"
                weight="semibold"
                color={isSelected ? 'steel-darker' : 'steel-dark'}
            >
                {domainName ?? formatAddress(address)}
            </Text>
            <div className="ml-auto">
                <Text variant="bodySmall" color="steel" weight="semibold" mono>
                    {totalAmount} {totalAmountSymbol}
                </Text>
            </div>
        </div>
    );
}
