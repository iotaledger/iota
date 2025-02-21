// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ThemeVars } from './themeContract.js';

export const darkTheme: ThemeVars = {
    blurs: {
        modalOverlay: 'blur(12px)',
    },
    backgroundColors: {
        primaryButton: '#253041',
        primaryButtonHover: '#313D50',
        outlineButtonHover: '#313D50',
        modalOverlay: 'rgba(0, 47, 109, 0.72)',
        modalPrimary: '#0f141c',
        modalSecondary: '#171d26',
        iconButton: 'transparent',
        dropdownMenu: '#0f141c',
        dropdownMenuSeparator: '#bed8ff14',
        walletItemSelected: 'rgba(190, 216, 255, 0.12)',
        walletItemHover: 'rgba(190, 216, 255, 0.06)',
        scrollThumb: '#3c4656',
    },
    borderColors: {
        outlineButton: '#8892A1',
    },
    colors: {
        primaryButton: '#E3EAF6',
        outlineButtonHover: '#171D26',
        iconButton: '#E3EAF6',
        body: '#E3EAF6',
        bodyMuted: '#8892A1',
        bodyDanger: '#FFB0BE',
    },
    radii: {
        small: '6px',
        medium: '8px',
        large: '12px',
        xlarge: '16px',
        full: '120px',
    },
    fontWeights: {
        normal: '400',
        medium: '500',
        bold: '600',
    },
    fontSizes: {
        small: '14px',
        medium: '16px',
        large: '18px',
        xlarge: '20px',
    },
    typography: {
        fontFamily:
            'ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans", sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"',
        fontStyle: 'normal',
        lineHeight: '24px',
        letterSpacing: '0.1px',
    },
    spacing: {
        xxsmall: '4px',
        xsmall: '8px',
        small: '12px',
        medium: '16px',
        large: '24px',
        xlarge: '32px',
    },
};
