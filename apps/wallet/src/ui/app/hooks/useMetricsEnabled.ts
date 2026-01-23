// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';

export const METRICS_STORAGE_KEY = 'metrics_enabled_iota-wallet';
const DEFAULT_METRICS_ENABLED = true;

export function useMetricsEnabled(): [boolean, (enabled: boolean) => void] {
    const [isEnabled, setIsEnabled] = useState<boolean>(() => {
        const storedValue = localStorage.getItem(METRICS_STORAGE_KEY);
        return storedValue ? JSON.parse(storedValue) : DEFAULT_METRICS_ENABLED;
    });

    const setMetricsEnabled = (enabled: boolean) => {
        localStorage.setItem(METRICS_STORAGE_KEY, JSON.stringify(enabled));
        setIsEnabled(enabled);
    };

    return [isEnabled, setMetricsEnabled];
}
