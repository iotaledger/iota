// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { COIN_GECKO_IOTA_URL, useIotaCoinData } from '@iota/core';
import { Iota } from '@iota/icons';
import { Text } from '@iota/ui';

import { ButtonOrLink, Card } from '~/components/ui';

export function IotaTokenCard(): JSX.Element {
    const { data } = useIotaCoinData();
    const { currentPrice } = data || {};

    const formattedPrice = currentPrice
        ? currentPrice.toLocaleString('en', {
              style: 'currency',
              currency: 'USD',
          })
        : '--';

    return (
        <ButtonOrLink href={COIN_GECKO_IOTA_URL}>
            <Card growOnHover bg="white/80" spacing="lg" height="full">
                <div className="flex items-center gap-2">
                    <div className="h-5 w-5 flex-shrink-0 rounded-full bg-iota p-1">
                        <Iota className="h-full w-full text-white" />
                    </div>
                    <div className="flex w-full flex-col gap-0.5">
                        <Text variant="body/semibold" color="steel-darker">
                            1 IOTA = {formattedPrice}
                        </Text>
                        <Text variant="subtitleSmallExtra/medium" color="steel">
                            via CoinGecko
                        </Text>
                    </div>
                </div>
            </Card>
        </ButtonOrLink>
    );
}
