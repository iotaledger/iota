// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AppType } from '_src/ui/app/redux/slices/app/appType';
import { openInNewTab } from '_shared/utils';
import { useEffect, useRef } from 'react';

import useAppSelector from './useAppSelector';

export default function useFullscreenGuard(enabled: boolean) {
    const appType = useAppSelector((state) => state.app.appType);
    const isOpenTabInProgressRef = useRef(false);
    useEffect(() => {
        if (enabled && appType === AppType.Popup && !isOpenTabInProgressRef.current) {
            isOpenTabInProgressRef.current = true;
            openInNewTab().finally(() => window.close());
        }
    }, [appType, enabled]);
    return !enabled && appType === AppType.Unknown;
}
