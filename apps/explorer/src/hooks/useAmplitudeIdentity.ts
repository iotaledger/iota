// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { setAmplitudeIdentity } from '~/lib/utils/analytics/amplitude';

/**
 * Hook that keeps Amplitude user identity in sync with the current network.
 * Automatically updates identity when network changes, ensuring all
 * subsequent events include the latest network context.
 */
export function useAmplitudeIdentity(network: string): void {
    useEffect(() => {
        setAmplitudeIdentity(network);
    }, [network]);
}
