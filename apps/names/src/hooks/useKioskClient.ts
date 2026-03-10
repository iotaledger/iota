// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext } from 'react';

import { KioskClientContext } from '@/contexts/KioskClientContext';

export function useKioskClient() {
    const kioskClient = useContext(KioskClientContext);
    if (!kioskClient) {
        throw new Error(
            'KioskClientContext not available. Make sure your component is wrapped in a KioskClientProvider.',
        );
    }
    return kioskClient;
}
