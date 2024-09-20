// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';
import { themeVars } from '../../../themes/themeContract.js';

export const container = style({
    display: 'flex',
    flexDirection: 'column',
    gap: themeVars.spacing.xs,
    height: '100%',
    width: '100%',
    maxHeight: '330px',
});
