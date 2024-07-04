// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_GAS_COIN_TYPE } from '_redux/slices/iota-objects/Coin';
import { useFormatCoin } from '@iota/core';

export interface FaucetMessageInfoProps {
    error?: string | null;
    loading?: boolean;
    totalReceived?: number | null;
}

function FaucetMessageInfo({
    error = null,
    loading = false,
    totalReceived = null,
}: FaucetMessageInfoProps) {
    const [coinsReceivedFormatted, coinsReceivedSymbol] = useFormatCoin(
        totalReceived,
        IOTA_GAS_COIN_TYPE,
    );
    if (loading) {
        return <>Request in progress</>;
    }
    if (error) {
        return <>{error}</>;
    }
    return (
        <>{`${totalReceived ? `${coinsReceivedFormatted} ` : ''}${coinsReceivedSymbol} received`}</>
    );
}

export default FaucetMessageInfo;
