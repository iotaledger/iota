// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BackgroundClient } from '_app/background-client';
import { growthbook } from '_src/ui/app/experimentation/featureGating';
import type { RootState } from '_src/ui/app/redux/rootReducer';
import type { AppDispatch } from '_store';

export const thunkExtras = {
    growthbook,
    background: new BackgroundClient(),
};

type ThunkExtras = typeof thunkExtras;

export interface AppThunkConfig {
    extra: ThunkExtras;
    state: RootState;
    dispatch: AppDispatch;
}
