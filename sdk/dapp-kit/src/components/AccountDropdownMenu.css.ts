// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';

import { themeVars } from '../themes/themeContract.js';

export const connectedAccount = style({
    gap: 8,
});

export const menuContainer = style({
    zIndex: 999999999,
});

export const icon = style({
   color: themeVars.colors.body,
});

export const menuContent = style({
    display: 'flex',
    flexDirection: 'column',
    width: 180,
    maxHeight: 200,
    marginTop: themeVars.spacing.xsmall,
    gap: themeVars.spacing.xxsmall,
    borderRadius: themeVars.radii.large,
    backgroundColor: themeVars.backgroundColors.dropdownMenu,
    padding: themeVars.spacing.xxsmall,
});

export const menuItem = style({
    userSelect: 'none',
    outline: 'none',
    display: 'flex',
    alignItems: 'center',
    borderRadius: themeVars.radii.medium,
    fontSize: themeVars.fontSizes.medium,
    fontWeight: themeVars.fontWeights.normal,
    letterSpacing: themeVars.typography.letterSpacing,
    lineHeight: themeVars.typography.lineHeight,
    padding: themeVars.spacing.small,
    ':hover': {
        backgroundColor: themeVars.backgroundColors.primaryButtonHover,
    },
});

export const switchAccountMenuItem = style({
    display: 'flex',
    justifyContent: 'space-between',
    alignItems: 'center',
});

export const separator = style({
    height: 1,
    flexShrink: 0,
    backgroundColor: themeVars.backgroundColors.dropdownMenuSeparator,
});
