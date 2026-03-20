// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const BUILDER_MODE_STORAGE_KEY = 'builder_mode_iota-wallet';

export function getBuilderMode(): boolean {
    return localStorage.getItem(BUILDER_MODE_STORAGE_KEY) === 'true';
}

export function setBuilderMode(enabled: boolean): void {
    localStorage.setItem(BUILDER_MODE_STORAGE_KEY, String(enabled));
}

export function useBuilderMode(): [boolean, (enabled: boolean) => void] {
    // Read directly from localStorage — same pattern as ThemeProvider.
    // The settings toggle re-renders the component via its own state, so no
    // additional subscription is needed here.
    const isBuilderMode = getBuilderMode();
    return [isBuilderMode, setBuilderMode];
}
