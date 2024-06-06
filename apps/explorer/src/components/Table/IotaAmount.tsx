// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { Text } from '@iota/ui';

export function IotaAmount({
    amount,
    full = false,
}: {
    amount?: bigint | number | string | null;
    full?: boolean;
}) {
    const [formattedAmount, coinType] = useFormatCoin(
        amount,
        IOTA_TYPE_ARG,
        full ? CoinFormat.FULL : CoinFormat.ROUNDED,
    );
    if (!amount) return <Text variant="bodySmall/medium">--</Text>;

    return (
        <div className="leading-1 flex items-end gap-0.5">
            <Text variant="bodySmall/medium" color="steel-darker">
                {formattedAmount}
            </Text>
            <Text variant="captionSmall/normal" color="steel-dark">
                {coinType}
            </Text>
        </div>
    );
}
