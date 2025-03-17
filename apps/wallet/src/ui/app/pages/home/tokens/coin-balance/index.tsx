// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    CoinFormat,
    formatBalance,
    formatBalanceToUSD,
    useBalanceInUSD,
    useFormatCoin,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import { Tooltip, TooltipPosition } from '@iota/apps-ui-kit';
import BigNumber from 'bignumber.js';
import { useAppSelector } from '_src/ui/app/hooks';

export interface CoinProps {
    type: string;
    amount: bigint;
}

interface WalletBalanceUsdProps {
    amount: bigint;
}

function WalletBalanceUsd({ amount: walletBalance }: WalletBalanceUsdProps) {
    const network = useAppSelector((state) => state.app.network);
    const formattedWalletBalance = useBalanceInUSD(IOTA_TYPE_ARG, walletBalance, network);

    const walletBalanceInUsd = useMemo(() => {
        if (!formattedWalletBalance) return null;

        return `~${formatBalanceToUSD(formattedWalletBalance)} USD`;
    }, [formattedWalletBalance]);

    if (!walletBalanceInUsd) {
        return null;
    }

    return (
        <div className="text-label-md text-neutral-40 dark:text-neutral-60">
            {walletBalanceInUsd}
        </div>
    );
}

export function CoinBalance({ amount: walletBalance, type }: CoinProps) {
    const [formatted, symbol, { data: coinMetadata }] = useFormatCoin({
        balance: walletBalance,
        coinType: type,
    });

    const iotaDecimals = coinMetadata?.decimals ?? 9;
    const bnBalance = new BigNumber(walletBalance.toString()).shiftedBy(-1 * iotaDecimals);
    const shouldShowTooltip = bnBalance.gt(0) && bnBalance.lt(1);

    return (
        <>
            <div className="flex items-baseline gap-0.5">
                {shouldShowTooltip ? (
                    <Tooltip
                        text={formatBalance(
                            walletBalance,
                            coinMetadata?.decimals ?? 9,
                            CoinFormat.FULL,
                        )}
                        position={TooltipPosition.Bottom}
                    >
                        <div
                            className="text-headline-lg text-neutral-10 dark:text-neutral-92"
                            data-testid="coin-balance"
                        >
                            {formatted}
                        </div>
                    </Tooltip>
                ) : (
                    <div
                        className="text-headline-lg text-neutral-10 dark:text-neutral-92"
                        data-testid="coin-balance"
                    >
                        {formatted}
                    </div>
                )}
                <div className="text-label-md text-neutral-40 dark:text-neutral-60">{symbol}</div>
            </div>
            <WalletBalanceUsd amount={walletBalance} />
        </>
    );
}
