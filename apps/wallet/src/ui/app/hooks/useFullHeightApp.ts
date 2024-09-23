// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { setFullHeight } from '_redux/slices/app';
import store from '_store';

/**
 * Sets the app to use the full height of the window.
 */
export default function useFullHeightApp() {
    useEffect(() => {
        store.dispatch(setFullHeight(true));
        return () => {
            store.dispatch(setFullHeight(false));
        };
    }, []);
}
