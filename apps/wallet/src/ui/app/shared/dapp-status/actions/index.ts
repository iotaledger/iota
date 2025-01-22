// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { AppThunkConfig } from '_src/ui/app/redux/store/thunkExtras';
import { createAsyncThunk } from '@reduxjs/toolkit';

export const appDisconnect = createAsyncThunk<
    void,
    { origin: string; accounts: string[] },
    AppThunkConfig
>('dapp-status-app-disconnect', async ({ origin, accounts }, { extra: { background } }) => {
    await background.disconnectApp(origin, accounts);
    await background.sendGetPermissionRequests();
});
