// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';

export interface FaucetMessageInfoProps {
    error?: string | null;
    loading?: boolean;
    totalReceived?: number | null;
}

export function FaucetMessageInfo({
    error = null,
    loading = false,
    totalReceived = null,
}: FaucetMessageInfoProps) {
    const [coinsReceivedFormatted, coinsReceivedSymbol] = useFormatCoin({ balance: totalReceived });
    if (loading) {
        return <>Request in progress</>;
    }
    if (error) {
        return <>{error}</>;
    }
    return (
        <>{`${totalReceived ? `${coinsReceivedFormatted} ` : ''}${coinsReceivedSymbol} requested`}</>
    );
}
