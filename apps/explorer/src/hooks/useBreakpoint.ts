// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMediaQuery } from '~/hooks/useMediaQuery';

/**
 * values taken from tailwind.config.js
 */
export const BREAK_POINT = {
    sm: 768,
    md: 1024,
    lg: 1400,
    xl: 1920,
};

export function useBreakpoint(breakpoint: keyof typeof BREAK_POINT): boolean {
    return useMediaQuery(`(min-width: ${BREAK_POINT[breakpoint]}px)`);
}
