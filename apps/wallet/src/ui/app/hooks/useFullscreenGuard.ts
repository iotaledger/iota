// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';
import { openInNewTab } from '_shared/utils';
import { useEffect, useRef } from 'react';
import { useAppSelector } from './useAppSelector';

export function useFullscreenGuard(enabled: boolean) {
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    const isOpenTabInProgressRef = useRef(false);
    useEffect(() => {
        if (
            enabled &&
            extensionViewType === ExtensionViewType.Popup &&
            !isOpenTabInProgressRef.current
        ) {
            isOpenTabInProgressRef.current = true;
            openInNewTab().finally(() => window.close());
        }
    }, [extensionViewType, enabled]);
    return !enabled && extensionViewType === ExtensionViewType.Unknown;
}
