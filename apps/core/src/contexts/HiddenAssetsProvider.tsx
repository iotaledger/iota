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
    undoHideAsset: (assetId: string) => Promise<void>;
    showAsset: (assetId: string) => Promise<string | void>;
    undoShowAsset: (assetId: string) => Promise<void>;
}

export const HiddenAssetsContext = createContext<HiddenAssetContext>({
    hiddenAssets: {
        type: 'loading',
    },
    setHiddenAssetIds: () => {},
    hideAsset: async () => {},
    undoHideAsset: async () => {},
    showAsset: async () => {},
    undoShowAsset: async () => {},
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

    const hideAssetId = useCallback(
        async (newAssetId: string) => {
            const newHiddenAssetIds = Array.from(
                new Set([...hiddenAssetIdsRef.current, newAssetId]),
            );
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
            // Ensure the asset exists in the hidden list
            if (!hiddenAssetIdsRef.current.includes(newAssetId)) return;

            // Compute the new list of hidden assets
            const updatedHiddenAssetIds = hiddenAssetIdsRef.current.filter(
                (id) => id !== newAssetId,
            );
            // Update the ref and state
            hiddenAssetIdsRef.current = updatedHiddenAssetIds;

            await set(HIDDEN_ASSET_IDS, updatedHiddenAssetIds);
            setHiddenAssetIds(updatedHiddenAssetIds);

            // try {
            //     const updatedHiddenAssetIds = hiddenAssetIds.filter((id) => id !== newAssetId);
            //     return newAssetId;
            // } catch (error) {
            //     // Restore the asset ID back to the hidden asset IDs list
            //     await set(HIDDEN_ASSET_IDS, hiddenAssetIds);
            //     setHiddenAssetIds([...hiddenAssetIds, newAssetId]);
            // }
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
