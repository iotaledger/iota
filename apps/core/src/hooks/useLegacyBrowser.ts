// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { isLegacyBrowser } from '../utils';
import { toast } from '../components/toaster';

export function useLegacyBrowser() {
    useEffect(() => {
        if (typeof window !== 'undefined' && isLegacyBrowser()) {
            toast.warning(
                'Your browser version is outdated. Please update it to the latest version.',
            );
        }
    }, []);
}
