// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { globalStyle } from '@vanilla-extract/css';

import { themeVars } from '../../themes/themeContract.js';

globalStyle(':where(*)', {
    boxSizing: 'border-box',
    color: themeVars.colors.body,
    fontFamily: themeVars.typography.fontFamily,
    fontSize: themeVars.fontWeights.normal,
    fontStyle: themeVars.typography.fontStyle,
    fontWeight: themeVars.fontWeights.normal,
    lineHeight: themeVars.typography.lineHeight,
    letterSpacing: themeVars.typography.letterSpacing,
});

globalStyle(':where(button)', {
    appearance: 'none',
    backgroundColor: 'transparent',
    fontSize: 'inherit',
    fontFamily: 'inherit',
    lineHeight: 'inherit',
    letterSpacing: 'inherit',
    color: 'inherit',
    border: 0,
    padding: 0,
    margin: 0,
});

globalStyle(':where(a)', {
    textDecoration: 'none',
    color: 'inherit',
    outline: 'none',
});

globalStyle(':where(ol, ul)', {
    listStyle: 'none',
    margin: 0,
    padding: 0,
});

globalStyle(':where(h1, h2, h3, h4, h5, h6)', {
    fontSize: 'inherit',
    fontWeight: 'inherit',
    margin: 0,
});

globalStyle('::-webkit-scrollbar', {
    width: '14px',
});

globalStyle('::-webkit-scrollbar-track', {
    borderRadius: themeVars.radii.xlarge,
    backgroundColor: 'transparent',
});

globalStyle('::-webkit-scrollbar-thumb', {
    boxShadow: `inset 0 0 10px 10px ${themeVars.backgroundColors.scrollThumb}`,
    borderRadius: themeVars.radii.xlarge,
    border: '4px solid transparent',
});
