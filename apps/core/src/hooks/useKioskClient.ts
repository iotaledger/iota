// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext } from 'react';

import { KioskClientContext } from '../components';

export function useKioskClient() {
    const kioskClient = useContext(KioskClientContext);
    if (!kioskClient) {
        throw new Error('Kiosk client not found. Please make sure KioskClientProvider is set up.');
    }
    return kioskClient;
}
