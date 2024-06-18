// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { get, set } from 'idb-keyval';
import React, { useCallback, useEffect, useState } from 'react';
import { HiddenAssetsContext } from '../contexts';

const HIDDEN_ASSET_IDS = 'hidden-asset-ids';

export default function HiddenAssetsProvider({
    children,
    onAssetHide = () => {},
    onAssetShow = () => {},
    onError = () => {},
}: React.PropsWithChildren<{
    onAssetHide?: (assetId: string, undoHideAsset: (assetId: string) => void) => void;
    onAssetShow?: (assetId: string, undoShowAsset: (assetId: string) => void) => void;
    onError?: (message: string) => void;
}>) {
    const [hiddenAssetIds, setHiddenAssetIds] = useState<string[]>([]);

    useEffect(() => {
        (async () => {
            const hiddenAssets = await get<string[]>(HIDDEN_ASSET_IDS);
            if (hiddenAssets) {
                setHiddenAssetIds(hiddenAssets);
            }
        })();
    }, []);

    const hideAssetId = useCallback(
        async (newAssetId: string) => {
            if (hiddenAssetIds.includes(newAssetId)) return;

            const newHiddenAssetIds = [...hiddenAssetIds, newAssetId];
            setHiddenAssetIds(newHiddenAssetIds);
            await set(HIDDEN_ASSET_IDS, newHiddenAssetIds);

            const undoHideAsset = async (assetId: string) => {
                try {
                    let updatedHiddenAssetIds;
                    setHiddenAssetIds((prevIds) => {
                        updatedHiddenAssetIds = prevIds.filter((id) => id !== assetId);
                        return updatedHiddenAssetIds;
                    });
                    await set(HIDDEN_ASSET_IDS, updatedHiddenAssetIds);
                } catch (error) {
                    // Handle any error that occurred during the unhide process
                    onError?.('Failed to unhide asset.');
                    // Restore the asset ID back to the hidden asset IDs list
                    setHiddenAssetIds([...hiddenAssetIds, assetId]);
                    await set(HIDDEN_ASSET_IDS, hiddenAssetIds);
                }
            };

            onAssetHide?.(newAssetId, undoHideAsset);
        },
        [hiddenAssetIds],
    );

    const showAssetId = useCallback(
        async (newAssetId: string) => {
            if (!hiddenAssetIds.includes(newAssetId)) return;

            try {
                const updatedHiddenAssetIds = hiddenAssetIds.filter((id) => id !== newAssetId);
                setHiddenAssetIds(updatedHiddenAssetIds);
                await set(HIDDEN_ASSET_IDS, updatedHiddenAssetIds);
            } catch (error) {
                // Handle any error that occurred during the unhide process
                onError?.('Failed to show asset.');
                // Restore the asset ID back to the hidden asset IDs list
                setHiddenAssetIds([...hiddenAssetIds, newAssetId]);
                await set(HIDDEN_ASSET_IDS, hiddenAssetIds);
            }

            const undoShowAsset = async (assetId: string) => {
                let newHiddenAssetIds;
                setHiddenAssetIds((prevIds) => {
                    return (newHiddenAssetIds = [...prevIds, assetId]);
                });
                await set(HIDDEN_ASSET_IDS, newHiddenAssetIds);
            };

            onAssetShow?.(newAssetId, undoShowAsset);
        },
        [hiddenAssetIds],
    );

    return (
        <HiddenAssetsContext.Provider
            value={{
                hiddenAssetIds: Array.from(new Set(hiddenAssetIds)),
                setHiddenAssetIds,
                hideAsset: hideAssetId,
                showAsset: showAssetId,
            }}
        >
            {children}
        </HiddenAssetsContext.Provider>
    );
}
