// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BackgroundClient } from '_app/background-client';
import { growthbook } from '_app/experimentation/feature-gating';
import type { RootState } from '_redux/RootReducer';
import type { AppDispatch } from '_store';

export const THUNK_EXTRAS = {
    growthbook,
    background: new BackgroundClient(),
};

type ThunkExtras = typeof THUNK_EXTRAS;

export interface AppThunkConfig {
    extra: ThunkExtras;
    state: RootState;
    dispatch: AppDispatch;
}
