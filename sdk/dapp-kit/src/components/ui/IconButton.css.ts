// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';

import { themeVars } from '../../themes/themeContract.js';

export const container = style({
    borderRadius: 9999,
    padding: 8,
    color: themeVars.colors.iconButton,
    backgroundColor: themeVars.backgroundColors.iconButton,
});
