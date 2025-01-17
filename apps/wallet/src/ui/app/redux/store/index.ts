// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import rootReducer from '_src/ui/app/redux/rootReducer';
import { configureStore } from '@reduxjs/toolkit';

import { thunkExtras } from './thunkExtras';

const store = configureStore({
    reducer: rootReducer,
    middleware: (getDefaultMiddleware) =>
        getDefaultMiddleware({
            thunk: {
                extraArgument: thunkExtras,
            },
        }),
});

export default store;

export type AppDispatch = typeof store.dispatch;
