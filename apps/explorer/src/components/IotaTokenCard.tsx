// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { COIN_GECKO_IOTA_URL, useIotaCoinData } from '@iota/core';
import { ButtonOrLink, Card, ImageIconSize } from '~/components/ui';
import { CoinIcon } from './owned-coins';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-shader-neutral-light-8 text-neutral-10">
                        <CoinIcon coinType={IOTA_TYPE_ARG} size={ImageIconSize.Small} />
                    </div>
                    <div className="flex w-full flex-col gap-0.5">
                        <span className="font-inter text-title-lg text-neutral-10 dark:text-neutral-92">
                            1 IOTA = {formattedPrice}
                        </span>
                        <span className="font-inter text-label-lg text-neutral-60 dark:text-neutral-40">
                            via CoinGecko
                        </span>
                    </div>
                </div>
            </Card>
        </ButtonOrLink>
    );
}
