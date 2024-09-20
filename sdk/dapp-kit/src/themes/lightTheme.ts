// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ThemeVars } from './themeContract.js';

export const lightTheme: ThemeVars = {
    blurs: {
        modalOverlay: 'blur(12px)',
    },
    backgroundColors: {
        primaryButton: '#DDE4F0',
        primaryButtonHover: '#002F6D14',
        outlineButtonHover: '#002F6D14',
        modalOverlay: 'rgba(0, 47, 109, 0.72)',
        modalPrimary: '#ffffff',
        modalSecondary: '#e3eaf6',
        iconButton: 'transparent',
        dropdownMenu: '#FFFFFF',
        dropdownMenuSeparator: '#002f6d14',
        walletItemSelected: '#EFF4FA',
        walletItemHover: 'rgba(0, 103, 238, 0.12)',
        scrollThumb: '#cad4e2',
    },
    borderColors: {
        outlineButton: '#6E7787',
    },
    colors: {
        primaryButton: '#171D26',
        outlineButton: '#171D26',
        iconButton: '#171D26',
        body: '#171D26',
        bodyMuted: '#545E6E',
        bodyDanger: '#B51431',
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
