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
    setHiddenAssetIds: (hiddenAssetIds: string[]) => void;
    hideAsset: (assetId: string) => Promise<string | void>;
    showAsset: (assetId: string) => Promise<string | void>;
}

export const HiddenAssetsContext = createContext<HiddenAssetContext>({
    hiddenAssets: {
        type: 'loading',
    },
    setHiddenAssetIds: () => {},
    hideAsset: async () => {},
    showAsset: async () => {},
});

export const HiddenAssetsProvider = ({ children }: PropsWithChildren) => {
    const [hiddenAssets, setHiddenAssets] = useState<HiddenAssets>({
        type: 'loading',
    });
    const hiddenAssetIdsRef = useRef<string[]>([]);

    const hiddenAssetIds = hiddenAssets.type === 'loaded' ? hiddenAssets.assetIds : [];

    useEffect(() => {
        (async () => {
            const hiddenAssetsFromStorage = (await get<string[]>(HIDDEN_ASSET_IDS)) ?? [];
            hiddenAssetIdsRef.current = hiddenAssetsFromStorage;
            setHiddenAssetIds(hiddenAssetsFromStorage);
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

    const hideAssetId = useCallback(
        async (newAssetId: string) => {
            const prevIds = [...hiddenAssetIdsRef.current];
            const newHiddenAssetIds = Array.from(
                new Set([...hiddenAssetIdsRef.current, newAssetId]),
            );
            setHiddenAssetIds(newHiddenAssetIds);
            syncIdb(newHiddenAssetIds, prevIds);
            return newAssetId;
        },
        [hiddenAssetIds],
    );

    const showAssetId = useCallback(
        async (newAssetId: string) => {
            // Ensure the asset exists in the hidden list
            if (!hiddenAssetIdsRef.current.includes(newAssetId)) return;

            const prevIds = [...hiddenAssetIdsRef.current];
            // Compute the new list of hidden assets
            const updatedHiddenAssetIds = hiddenAssetIdsRef.current.filter(
                (id) => id !== newAssetId,
            );
            setHiddenAssetIds(updatedHiddenAssetIds);
            syncIdb(updatedHiddenAssetIds, prevIds);
        },
        [hiddenAssetIds],
    );

    return (
        <HiddenAssetsContext.Provider
            value={{
                hiddenAssets:
                    hiddenAssets.type === 'loaded'
                        ? { ...hiddenAssets, assetIds: Array.from(new Set(hiddenAssetIds)) }
                        : { type: 'loading' },
                setHiddenAssetIds,
                hideAsset: hideAssetId,
                showAsset: showAssetId,
            }}
        >
            {children}
        </HiddenAssetsContext.Provider>
    );
};

export const useHiddenAssets = () => {
    return useContext(HiddenAssetsContext);
};
