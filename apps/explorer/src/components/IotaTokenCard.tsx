// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Panel } from '@iota/apps-ui-kit';
import {
    COIN_GECKO_IOTA_URL,
    CoinIcon,
    formatBalanceToUSD,
    ImageIconSize,
    useBalanceInUSD,
} from '@iota/core';
import { ButtonOrLink } from '~/components/ui';
import { IOTA_TYPE_ARG, NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { useIotaClientContext } from '@iota/dapp-kit';
import { type Network } from '@iota/iota-sdk/client';

export function IotaTokenCard(): JSX.Element {
    const { network } = useIotaClientContext();
    const iotaPrice = useBalanceInUSD(IOTA_TYPE_ARG, NANOS_PER_IOTA, network as Network);
    const formattedPrice = formatBalanceToUSD(iotaPrice ?? 0);

    return (
        <ButtonOrLink href={COIN_GECKO_IOTA_URL}>
            <Panel>
                <div className="flex items-center gap-xs p-md--rs">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-shader-neutral-light-8 text-neutral-10">
                        <CoinIcon coinType={IOTA_TYPE_ARG} size={ImageIconSize.Small} />
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
