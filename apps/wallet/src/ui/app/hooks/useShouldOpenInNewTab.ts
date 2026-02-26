// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NEW_TAB_ACCOUNT_TYPES } from '_src/shared/accountTypes';
import { useAppSelector } from './useAppSelector';
import { ExtensionViewType } from '../redux/slices/app/appType';
import { useActiveAccount } from './useActiveAccount';

export function useShouldOpenInNewTab() {
    const activeAccount = useActiveAccount();

    const isTabView = useAppSelector(
        (state) => state.app.extensionViewType === ExtensionViewType.FullScreen,
    );

    if (!activeAccount) return false;
    return NEW_TAB_ACCOUNT_TYPES.includes(activeAccount?.type) && !isTabView;
}
