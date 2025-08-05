// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LabelText, LabelTextSize } from '@iota/apps-ui-kit';
import { useFormatCoin } from '@iota/core';
import { CoinFormat } from '@iota/iota-sdk/utils';

type LabelTextProps = Omit<React.ComponentProps<typeof LabelText>, 'text' | 'size'>;

interface TokenStatsProps extends LabelTextProps {
    amount: bigint | number | string | undefined | null;
    showSign?: boolean;
    size?: LabelTextSize;
}

export function TokenStats({
    amount,
    showSign,
    size = LabelTextSize.Large,
    ...props
}: TokenStatsProps): React.JSX.Element {
    const [formattedAmount, symbol] = useFormatCoin({
        balance: amount,
        format: CoinFormat.Rounded,
        showSign,
    });

    return <LabelText text={formattedAmount} supportingLabel={symbol} size={size} {...props} />;
}
