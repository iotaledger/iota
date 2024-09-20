// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';
import { themeVars } from '../../../themes/themeContract.js';

export const walletConnectContainer = style({
    height: '100%',
    width: '100%',
    overflow: 'auto',
});

export const walletConnectFooter = style({
    display: 'flex',
    flexDirection: 'column',
    gap: themeVars.spacing.sm,
});

export const errorContainer = style({
    display: 'flex',
    width: '100%',
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: themeVars.spacing.sm,
});

export const errorMessage = style({
    fontSize: themeVars.fontSizes.small,
    color: themeVars.colors.bodyDanger,
});

export const successContainer = style({
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    gap: themeVars.spacing.sm,
});

export const openingText = style({
    fontSize: themeVars.fontSizes.small,
    color: themeVars.colors.bodyMuted,
});

export const confirmText = style({
    fontSize: themeVars.fontSizes.medium,
    color: themeVars.colors.body,
});

export const separator = style({
    height: 1,
    backgroundColor: themeVars.backgroundColors.dropdownMenuSeparator,
    width: '100%',
});
