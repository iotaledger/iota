// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppSelector } from './useAppSelector';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';

export function useSidebar(): boolean {
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    return (
        extensionViewType === ExtensionViewType.FullScreen ||
        extensionViewType === ExtensionViewType.Popup
    );
}
