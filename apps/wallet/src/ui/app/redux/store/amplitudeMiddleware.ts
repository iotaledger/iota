// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { setAmplitudeIdentity } from '_src/shared/analytics/amplitude';
import type { RootState } from '_src/ui/app/redux/rootReducer';
import type { Middleware } from '@reduxjs/toolkit';

/**
 * Redux middleware that keeps the Amplitude user identity in sync with the
 * Redux `app` slice.  It runs synchronously as part of every dispatch, *after*
 * the reducer has committed the new state.  This guarantees that any
 * Amplitude event fired later in the same tick (e.g. `ampli.switchedNetwork`
 * after an `await dispatch(…).unwrap()`) already carries the updated identity.
 *
 * Only the fields that are reflected in the Amplitude identity are compared;
 * unrelated dispatches are a no-op.
 */
export const amplitudeMiddleware: Middleware<{}, RootState> = (storeAPI) => (next) => (action) => {
    const { network, customRpc, appType } = storeAPI.getState().app;

    const result = next(action);

    const newApp = storeAPI.getState().app;
    if (
        newApp.network !== network ||
        newApp.customRpc !== customRpc ||
        newApp.appType !== appType
    ) {
        setAmplitudeIdentity();
    }

    return result;
};
