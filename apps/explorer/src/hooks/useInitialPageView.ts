// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';

import { setAmplitudeIdentity } from '~/lib/utils';

export function useInitialPageView(activeNetwork: string): void {
    const location = useLocation();

    // Set user properties for the user's page information
    useEffect(() => {
        // Wait 1.2s to ensure initAmplitude has finished loading (avoids race conditions)
        const timer = setTimeout(() => {
            setAmplitudeIdentity(activeNetwork);
        }, 1200); // 1.2 seconds (giving 200ms buffer over the init)
        return () => clearTimeout(timer);
    }, [location.pathname, activeNetwork]);
}
