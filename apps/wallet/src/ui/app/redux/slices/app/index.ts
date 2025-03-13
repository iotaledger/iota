// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { walletApiProvider } from '_src/ui/app/apiProvider';
import type { RootState } from '_src/ui/app/redux/rootReducer';
import type { NetworkEnvType } from '@iota/core';
import type { AppThunkConfig } from '_src/ui/app/redux/store/thunkExtras';
import { getDefaultNetwork, type Network } from '@iota/iota-sdk/client';
import { createAsyncThunk, createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';

import { AppType } from './appType';

type AppState = {
    appType: AppType;
    network: Network;
    customRpc: string | null;
    navVisible: boolean;
    activeOrigin: string | null;
    activeOriginFavIcon: string | null;
    isAppViewPopup: boolean | null;
};

const initialState: AppState = {
    appType: AppType.Unknown,
    network: getDefaultNetwork(),
    customRpc: null,
    navVisible: true,
    activeOrigin: null,
    activeOriginFavIcon: null,
    isAppViewPopup: null,
};

export const changeActiveNetwork = createAsyncThunk<
    void,
    { network: NetworkEnvType; store?: boolean },
    AppThunkConfig
>('changeRPCNetwork', async ({ network, store = false }, { extra: { background }, dispatch }) => {
    if (store) {
        await background.setActiveNetworkEnv(network);
    }
    walletApiProvider.setNewJsonRpcProvider(network.network, network.customRpcUrl);
    await dispatch(slice.actions.setActiveNetwork(network));
});

const slice = createSlice({
    name: 'app',
    reducers: {
        initAppType: (state, { payload }: PayloadAction<AppType>) => {
            state.appType = payload;
        },
        setActiveNetwork: (
            state,
            { payload: { network, customRpcUrl } }: PayloadAction<NetworkEnvType>,
        ) => {
            state.network = network;
            state.customRpc = customRpcUrl;
        },
        setNavVisibility: (state, { payload: isVisible }: PayloadAction<boolean>) => {
            state.navVisible = isVisible;
        },
        setActiveOrigin: (
            state,
            { payload }: PayloadAction<{ origin: string | null; favIcon: string | null }>,
        ) => {
            state.activeOrigin = payload.origin;
            state.activeOriginFavIcon = payload.favIcon;
        },
        setIsAppViewPopup: (state, { payload }: PayloadAction<boolean>) => {
            state.isAppViewPopup = payload;
        },
    },
    initialState,
});

export const { initAppType, setNavVisibility, setActiveOrigin, setIsAppViewPopup } = slice.actions;
export const getNavIsVisible = ({ app }: RootState) => app.navVisible;

export default slice.reducer;
