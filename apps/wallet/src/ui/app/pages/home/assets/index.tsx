// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import { Route, Routes } from 'react-router-dom';

import { HiddenAssetsPage, NftsPage } from '..';
import { HiddenAssetsProvider } from '@iota/core';
import { showAssetHiddenToast, showAssetShownToast } from '_src/ui/app/helpers/hiddenAssets';

function AssetsPage() {
    if (useUnlockedGuard()) {
        return null;
    }
    return (
        <HiddenAssetsProvider onAssetHide={showAssetHiddenToast} onAssetShow={showAssetShownToast}>
            <Routes>
                <Route path="/hidden-assets" element={<HiddenAssetsPage />} />
                <Route path="/:filterType?/*" element={<NftsPage />} />
            </Routes>
        </HiddenAssetsProvider>
    );
}

export default AssetsPage;
