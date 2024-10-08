// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';

import { themeVars } from '../../themes/themeContract.js';

export const overlay = style({
    backgroundColor: themeVars.backgroundColors.modalOverlay,
    backdropFilter: themeVars.blurs.modalOverlay,
    position: 'fixed',
    inset: 0,
    zIndex: 999999999,
});

export const title = style({
    color: themeVars.colors.body,
    fontSize: themeVars.fontSizes.xlarge,
    fontWeight: themeVars.fontWeights.medium,
    margin: 0,
});

export const separator = style({
    height: 1,
    backgroundColor: themeVars.backgroundColors.dropdownMenuSeparator,
    width: '100%',
});

export const content = style({
    backgroundColor: themeVars.backgroundColors.modalPrimary,
    borderRadius: themeVars.radii.xlarge,
    color: themeVars.colors.body,
    position: 'fixed',
    top: 0,
    left: '50%',
    display: 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
    minHeight: '50vh',
    maxHeight: '85vh',
    width: '330px',
    transform: 'translate(-50%, 100%)',
    '@media': {
        'screen and (min-width: 768px)': {
            flexDirection: 'row',
            top: '50%',
            left: '50%',
            transform: 'translate(-50%, -50%)',
        },
    },
});

export const backButtonContainer = style({
    position: 'absolute',
    top: 20,
    left: 20,
    '@media': {
        'screen and (min-width: 768px)': {
            display: 'none',
        },
    },
});

export const closeButtonContainer = style({
    position: 'absolute',
    top: 16,
    right: 16,
});

export const walletListContent = style({
    display: 'flex',
    flexDirection: 'column',
    width: '100%',
    flexGrow: 1,
    gap: themeVars.spacing.medium,
    padding: themeVars.spacing.medium,
    backgroundColor: themeVars.backgroundColors.modalPrimary,
    '@media': {
        'screen and (min-width: 768px)': {
            backgroundColor: themeVars.backgroundColors.modalSecondary,
        },
    },
});
