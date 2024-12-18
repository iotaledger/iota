// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import rootReducer from '_redux/RootReducer';
import { configureStore } from '@reduxjs/toolkit';

import { thunkExtras } from './thunk-extras';

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
