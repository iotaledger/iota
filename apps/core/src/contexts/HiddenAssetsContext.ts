// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext } from 'react';

interface HiddenAssetContext {
    hiddenAssetIds: string[];
    setHiddenAssetIds: (hiddenAssetIds: string[]) => void;
    hideAsset: (assetId: string) => void;
    showAsset: (assetId: string) => void;
}

export const HiddenAssetsContext = createContext<HiddenAssetContext>({
    hiddenAssetIds: [],
    setHiddenAssetIds: () => {},
    hideAsset: () => {},
    showAsset: () => {},
});
