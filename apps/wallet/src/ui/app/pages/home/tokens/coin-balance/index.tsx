// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useAppSelector } from '_hooks';
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

    return <div className="text-label-md text-neutral-40">{walletBalanceInUsd}</div>;
}

export function CoinBalance({ amount: walletBalance, type }: CoinProps) {
    const network = useAppSelector((state) => state.app.network);
    const [formatted, symbol] = useFormatCoin(walletBalance, type);

    return (
        <>
            <div className="flex items-baseline gap-0.5">
                <div className="text-headline-lg text-neutral-10" data-testid="coin-balance">
                    {formatted}
                </div>
                <div className="text-label-md text-neutral-40">{symbol}</div>
            </div>
            {network === Network.Mainnet ? <WalletBalanceUsd amount={walletBalance} /> : null}
        </>
    );
}
