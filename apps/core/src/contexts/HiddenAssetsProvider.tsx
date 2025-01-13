// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { get, set } from 'idb-keyval';
import {
    PropsWithChildren,
    createContext,
    useCallback,
    useContext,
    useEffect,
    useState,
    useRef,
} from 'react';

const HIDDEN_ASSET_IDS = 'hidden-asset-ids';

export type HiddenAssets =
    | {
          type: 'loading';
      }
    | {
          type: 'loaded';
          assetIds: string[];
      };

interface HiddenAssetContext {
    hiddenAssets: HiddenAssets;
    hideAsset: (assetId: string) => string | void;
    showAsset: (assetId: string) => string | void;
}

export const HiddenAssetsContext = createContext<HiddenAssetContext>({
    hiddenAssets: {
        type: 'loading',
    },
    hideAsset: () => {},
    showAsset: () => {},
});

export const HiddenAssetsProvider = ({ children }: PropsWithChildren) => {
    const [hiddenAssets, setHiddenAssets] = useState<HiddenAssets>({
        type: 'loading',
    });
    const hiddenAssetIdsRef = useRef<string[]>([]);

    useEffect(() => {
        (async () => {
            try {
                const hiddenAssetsFromStorage = (await get<string[]>(HIDDEN_ASSET_IDS)) ?? [];
                hiddenAssetIdsRef.current = hiddenAssetsFromStorage;
                setHiddenAssetIds(hiddenAssetsFromStorage);
            } catch (error) {
                console.error('Failed to load hidden assets from storage:', error);
                setHiddenAssetIds([]);
            }
        })();
    }, []);

    function setHiddenAssetIds(hiddenAssetIds: string[]) {
        hiddenAssetIdsRef.current = hiddenAssetIds;
        setHiddenAssets({
            type: 'loaded',
            assetIds: hiddenAssetIds,
        });
    }

    const syncIdb = useCallback(async (nextState: string[], prevState: string[]) => {
        try {
            await set(HIDDEN_ASSET_IDS, nextState);
        } catch (error) {
            console.error('Error syncing with IndexedDB:', error);
            // Revert to the previous state on failure
            setHiddenAssetIds(prevState);
        }
    }, []);

    const hideAsset = useCallback((assetId: string) => {
        const prevIds = [...hiddenAssetIdsRef.current];
        const newHiddenAssetIds = Array.from(new Set([...hiddenAssetIdsRef.current, assetId]));
        setHiddenAssetIds(newHiddenAssetIds);
        syncIdb(newHiddenAssetIds, prevIds);
        return assetId;
    }, []);

    const showAsset = useCallback((assetId: string) => {
        // Ensure the asset exists in the hidden list
        if (!hiddenAssetIdsRef.current.includes(assetId)) return;

        const prevIds = [...hiddenAssetIdsRef.current];
        // Compute the new list of hidden assets
        const updatedHiddenAssetIds = hiddenAssetIdsRef.current.filter((id) => id !== assetId);
        setHiddenAssetIds(updatedHiddenAssetIds);
        syncIdb(updatedHiddenAssetIds, prevIds);
    }, []);

    return (
        <HiddenAssetsContext.Provider
            value={{
                hiddenAssets,
                hideAsset,
                showAsset,
            }}
        >
            {children}
        </HiddenAssetsContext.Provider>
    );
};

export const useHiddenAssets = () => {
    return useContext(HiddenAssetsContext);
};
