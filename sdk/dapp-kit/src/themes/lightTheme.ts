// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ThemeVars } from './themeContract.js';

export const lightTheme: ThemeVars = {
    blurs: {
        modalOverlay: 'blur(0)',
    },
    backgroundColors: {
        primaryButton: '#F6F7F9',
        primaryButtonHover: '#F0F2F5',
        outlineButtonHover: '#F4F4F5',
        modalOverlay: 'rgba(24 36 53 / 20%)',
        modalPrimary: 'white',
        modalSecondary: '#F7F8F8',
        iconButton: 'transparent',
        dropdownMenu: '#FFFFFF',
        dropdownMenuSeparator: '#F3F6F8',
        walletItemSelected: 'white',
        walletItemHover: '#3C424226',
        scrollThumb: '#cad4e2',
    },
    borderColors: {
        outlineButton: '#E4E4E7',
    },
    colors: {
        primaryButton: '#373737',
        outlineButtonHover: '#373737',
        iconButton: '#000000',
        body: '#182435',
        bodyMuted: '#767A81',
        bodyDanger: '#FF794B',
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
