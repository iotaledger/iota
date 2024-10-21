// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Panel } from '@iota/apps-ui-kit';
import { COIN_GECKO_IOTA_URL, useIotaCoinData } from '@iota/core';
import { IotaLogoMark } from '@iota/ui-icons';
import { ButtonOrLink } from '~/components/ui';

export function IotaTokenCard(): JSX.Element {
    const { data } = useIotaCoinData();
    const { currentPrice = 0.1219 } = data || {};

    const formattedPrice = currentPrice
        ? currentPrice.toLocaleString('en', {
              style: 'currency',
              currency: 'USD',
          })
        : '--';

    return (
        <ButtonOrLink href={COIN_GECKO_IOTA_URL}>
            <Panel>
                <div className="flex items-center gap-xs p-md--rs">
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary-30">
                        <IotaLogoMark className="h-5 w-5 text-white" />
                    </div>
                    <div className="flex w-full flex-col gap-xxxs">
                        <span className="font-inter text-title-lg text-neutral-10 dark:text-neutral-92">
                            1 IOTA = {formattedPrice}
                        </span>
                        <span className="font-inter text-label-lg text-neutral-60 dark:text-neutral-40">
                            via CoinGecko
                        </span>
                    </div>
                </div>
            </Panel>
        </ButtonOrLink>
    );
}
