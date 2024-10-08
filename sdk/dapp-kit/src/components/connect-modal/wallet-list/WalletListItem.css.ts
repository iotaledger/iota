// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';
import { themeVars } from '../../../themes/themeContract.js';

export const container = style({
    display: 'flex',
    width: '100%',
});

export const walletItem = style({
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    width: '100%',
    flexGrow: 1,
    padding: themeVars.spacing.medium,
    borderRadius: themeVars.radii.large,
    ':hover': {
        backgroundColor: themeVars.backgroundColors.primaryButtonHover,
    },
});

export const selectedWalletItem = style({
    border: `1px solid ${themeVars.borderColors.outlineButton}`,
    borderRadius: themeVars.radii.large,
});

export const walletName = style({
    display: 'flex',
    alignItems: 'center',
    flexDirection: 'row',
    gap: themeVars.spacing.small,
});

export const walletIcon = style({
    width: 28,
    height: 28,
    flexShrink: 0,
    objectFit: 'cover',
    borderRadius: themeVars.radii.small,
});
