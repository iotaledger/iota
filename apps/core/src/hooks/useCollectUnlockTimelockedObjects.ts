// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';

export function useCollectUnlockTimelockedObjects(address: string) {
    return useCallback(() => {
        console.log('Collecting!', address);
    }, []);
}
