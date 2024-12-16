// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { get, set } from 'idb-keyval';
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';

const HIDDEN_ASSET_IDS = 'hidden-asset-ids';

type HiddenAssets =
    | {
          type: 'loading';
      }
    | {
          type: 'loaded';
          assetIds: string[];
      };

interface HiddenAssetContext {
    hiddenAssets: HiddenAssets;
    setHiddenAssetIds: (hiddenAssetIds: string[]) => void;
    hideAsset: (assetId: string) => Promise<string | undefined>;
    undoHideAsset: (assetId: string) => Promise<void>;
    showAsset: (assetId: string) => Promise<string | undefined>;
    undoShowAsset: (assetId: string) => Promise<void>;
}

export const HiddenAssetsContext = createContext<HiddenAssetContext>({
    hiddenAssets: {
        type: 'loading',
    },
    setHiddenAssetIds: () => {},
    hideAsset: async () => undefined,
    undoHideAsset: async () => undefined,
    showAsset: async () => undefined,
    undoShowAsset: async () => undefined,
});

export const HiddenAssetsProvider = ({ children }: { children: ReactNode }) => {
    const [hiddenAssets, setHiddenAssets] = useState<HiddenAssets>({
        type: 'loading',
    });

    const hiddenAssetIds = hiddenAssets.type === 'loaded' ? hiddenAssets.assetIds : [];

    useEffect(() => {
        (async () => {
            const hiddenAssets = (await get<string[]>(HIDDEN_ASSET_IDS)) ?? [];
            setHiddenAssetIds(hiddenAssets);
        })();
    }, []);

    function setHiddenAssetIds(hiddenAssetIds: string[]) {
        setHiddenAssets({
            type: 'loaded',
            assetIds: hiddenAssetIds,
        });
    }

    const hideAssetId = useCallback(
        async (newAssetId: string) => {
            if (hiddenAssetIds.includes(newAssetId)) return;

            const newHiddenAssetIds = [...hiddenAssetIds, newAssetId];
            setHiddenAssetIds(newHiddenAssetIds);
            await set(HIDDEN_ASSET_IDS, newHiddenAssetIds);
            return newAssetId;
        },
        [hiddenAssetIds],
    );

    const undoHideAsset = async (assetId: string) => {
        try {
            let updatedHiddenAssetIds;
            setHiddenAssets((previous) => {
                const previousIds = previous.type === 'loaded' ? previous.assetIds : [];
                updatedHiddenAssetIds = previousIds.filter((id) => id !== assetId);
                return {
                    type: 'loaded',
                    assetIds: updatedHiddenAssetIds,
                };
            });
            await set(HIDDEN_ASSET_IDS, updatedHiddenAssetIds);
        } catch (error) {
            // Restore the asset ID back to the hidden asset IDs list
            setHiddenAssetIds([...hiddenAssetIds, assetId]);
            await set(HIDDEN_ASSET_IDS, hiddenAssetIds);
        }
    };

    const showAssetId = useCallback(
        async (newAssetId: string) => {
            if (!hiddenAssetIds.includes(newAssetId)) return;

            try {
                const updatedHiddenAssetIds = hiddenAssetIds.filter((id) => id !== newAssetId);
                setHiddenAssetIds(updatedHiddenAssetIds);
                await set(HIDDEN_ASSET_IDS, updatedHiddenAssetIds);
                return newAssetId;
            } catch (error) {
                // Restore the asset ID back to the hidden asset IDs list
                setHiddenAssetIds([...hiddenAssetIds, newAssetId]);
                await set(HIDDEN_ASSET_IDS, hiddenAssetIds);
            }
        },
        [hiddenAssetIds],
    );

    const undoShowAsset = async (assetId: string) => {
        let newHiddenAssetIds;
        setHiddenAssets((previous) => {
            const previousIds = previous.type === 'loaded' ? previous.assetIds : [];
            newHiddenAssetIds = [...previousIds, assetId];
            return {
                type: 'loaded',
                assetIds: newHiddenAssetIds,
            };
        });
        await set(HIDDEN_ASSET_IDS, newHiddenAssetIds);
    };

    return (
        <HiddenAssetsContext.Provider
            value={{
                hiddenAssets:
                    hiddenAssets.type === 'loaded'
                        ? { ...hiddenAssets, assetIds: Array.from(new Set(hiddenAssetIds)) }
                        : { type: 'loading' },
                setHiddenAssetIds,
                hideAsset: hideAssetId,
                undoHideAsset: undoHideAsset,
                showAsset: showAssetId,
                undoShowAsset: undoShowAsset,
            }}
        >
            {children}
        </HiddenAssetsContext.Provider>
    );
};

export const useHiddenAssets = () => {
    return useContext(HiddenAssetsContext);
};
