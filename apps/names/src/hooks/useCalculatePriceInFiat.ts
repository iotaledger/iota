// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';
import { Network } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { useBalanceInUSD } from '@/hooks';

// Fiat price calculation
export function useCalculatePriceInFiat(priceNanos: bigint | string | number): string {
    const { network } = useIotaClientContext();
    const iotaPrice = useBalanceInUSD(IOTA_TYPE_ARG, priceNanos, network as Network);
    return iotaPrice?.toFixed(2).toString() || '';
}
