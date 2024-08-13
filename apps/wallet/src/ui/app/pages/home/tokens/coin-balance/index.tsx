// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useIsWalletDefiEnabled } from '_app/hooks/useIsWalletDefiEnabled';
import { useAppSelector } from '_hooks';
import { Heading } from '_src/ui/app/shared/heading';
import { Text } from '_src/ui/app/shared/text';
import { useBalanceInUSD, useFormatCoin } from '@iota/core';
import { Network } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';

export interface CoinProps {
    type: string;
    amount: bigint;
}

interface WalletBalanceUsdProps {
    amount: bigint;
}

function WalletBalanceUsd({ amount: walletBalance }: WalletBalanceUsdProps) {
    const isDefiWalletEnabled = useIsWalletDefiEnabled();
    const formattedWalletBalance = useBalanceInUSD(IOTA_TYPE_ARG, walletBalance);

    const walletBalanceInUsd = useMemo(() => {
        if (!formattedWalletBalance) return null;

        return `~${formattedWalletBalance.toLocaleString('en', {
            style: 'currency',
            currency: 'USD',
        })} USD`;
    }, [formattedWalletBalance]);

    if (!walletBalanceInUsd) {
        return null;
    }

    return (
        <Text
            variant="caption"
            weight="medium"
            color={isDefiWalletEnabled ? 'hero-darkest' : 'steel'}
        >
            {walletBalanceInUsd}
        </Text>
    );
}

export function CoinBalance({ amount: walletBalance, type }: CoinProps) {
    const network = useAppSelector((state) => state.app.network);
    const [formatted, symbol] = useFormatCoin(walletBalance, type);

    return (
        <div className="flex flex-col items-center justify-center gap-1">
            <div className="flex items-center justify-center gap-2">
                <Heading leading="none" variant="heading1" weight="bold" color="gray-90">
                    {formatted}
                </Heading>

                <Heading variant="heading6" weight="medium" color="steel">
                    {symbol}
                </Heading>
            </div>
            <div>
                {network === Network.Mainnet ? <WalletBalanceUsd amount={walletBalance} /> : null}
            </div>
        </div>
    );
}
