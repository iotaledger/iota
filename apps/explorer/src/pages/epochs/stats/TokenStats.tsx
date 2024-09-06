// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LabelText, LabelTextSize } from '@iota/apps-ui-kit';
import { CoinFormat, useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/src/utils';

export function TokenStats({
    amount,
    showSign,
    size = LabelTextSize.Large,
    ...props
}: Omit<React.ComponentProps<typeof LabelText>, 'text'> & {
    amount: bigint | number | string | undefined | null;
    showSign?: boolean;
}): JSX.Element {
    const [formattedAmount, symbol] = useFormatCoin(
        amount,
        IOTA_TYPE_ARG,
        CoinFormat.ROUNDED,
        showSign,
    );

    return <LabelText text={formattedAmount} supportingLabel={symbol} size={size} {...props} />;
}
