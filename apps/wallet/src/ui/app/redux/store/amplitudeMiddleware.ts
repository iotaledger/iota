// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { setAmplitudeIdentity } from '_src/shared/analytics/amplitude';
import type { RootState } from '_src/ui/app/redux/rootReducer';
import type { Middleware } from '@reduxjs/toolkit';

/**
 * Redux middleware that keeps the Amplitude user identity in sync with Redux state.
 */
export const amplitudeMiddleware: Middleware<{}, RootState> = (storeAPI) => (next) => (action) => {
    const { network, customRpc, extensionViewType } = storeAPI.getState().app;

    const result = next(action);

    const newApp = storeAPI.getState().app;
    if (
        newApp.network !== network ||
        newApp.customRpc !== customRpc ||
        newApp.extensionViewType !== extensionViewType
    ) {
        setAmplitudeIdentity({
            network: newApp.network,
            customRpc: newApp.customRpc,
            extensionViewType: newApp.extensionViewType,
        });
    }

    return result;
};
