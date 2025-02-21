// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';
import { themeVars } from '../../../themes/themeContract.js';

export const container = style({
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    padding: themeVars.spacing.medium,
    height: '100%',
    gap: themeVars.spacing.xlarge,
});

export const content = style({
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: themeVars.spacing.medium,
});

export const text = style({
    fontSize: themeVars.fontSizes.medium,
    textAlign: 'center',
    color: themeVars.colors.body,
});

export const button = style({
    display: 'flex',
    alignItems: 'center',
    gap: themeVars.spacing.xsmall,
});

export const icon = style({
    color: themeVars.colors.body,
});
