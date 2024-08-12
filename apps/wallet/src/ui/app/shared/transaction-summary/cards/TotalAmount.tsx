// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Heading } from '_src/ui/app/shared/heading';
import { Text } from '_src/ui/app/shared/text';
import { useFormatCoin } from '@iota/core';

import { Card } from '_app/shared/transaction-summary/Card';

interface TotalAmountProps {
    amount?: string;
    coinType?: string;
}

export function TotalAmount({ amount, coinType }: TotalAmountProps) {
    const [formatted, symbol] = useFormatCoin(amount, coinType);
    if (!amount) return null;
    return (
        <Card>
            <div className="flex items-center justify-between">
                <Text color="steel-darker" variant="pBody">
                    Total Amount
                </Text>
                <div className="flex items-center gap-0.5">
                    <Heading color="steel-darker" variant="heading2">
                        {formatted}
                    </Heading>
                    <Text color="steel-darker" variant="body" weight="medium">
                        {symbol}
                    </Text>
                </div>
            </div>
        </Card>
    );
}
