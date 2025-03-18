// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { style } from '@vanilla-extract/css';

export const container = style({
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    overflowY: 'auto',
});

export const content = style({
    display: 'flex',
    flexDirection: 'column',
    justifyContent: 'center',
    flexGrow: 1,
    gap: 20,
    padding: 40,
    paddingBottom: 60,
    overflow: 'auto',
});

export const installButtonContainer = style({
    position: 'absolute',
    bottom: 20,
    right: 20,
});
