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

export const menuContent = style({
    display: 'flex',
    flexDirection: 'column',
    width: 180,
    maxHeight: 300,
    marginTop: 4,
    padding: 8,
    gap: 8,
    borderRadius: themeVars.radii.large,
    backgroundColor: themeVars.backgroundColors.dropdownMenu,
});

export const scrollableContent = style({
    overflowY: 'auto',
    maxHeight: 300,
    flexGrow: 1,
});

export const menuItem = style({
    padding: 8,
    height: 40,
    userSelect: 'none',
    outline: 'none',
    display: 'flex',
    alignItems: 'center',
    borderRadius: themeVars.radii.large,
    selectors: {
        '&[data-highlighted]': {
            backgroundColor: themeVars.backgroundColors.primaryButton,
        },
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
