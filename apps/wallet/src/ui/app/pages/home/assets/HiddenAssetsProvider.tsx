// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { get, set } from 'idb-keyval';
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import { type Toast, toast } from 'react-hot-toast';
import { ButtonUnstyled } from '@iota/apps-ui-kit';

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
    hideAsset: (assetId: string) => void;
    showAsset: (assetId: string) => void;
}

export const HiddenAssetsContext = createContext<HiddenAssetContext>({
    hiddenAssets: {
        type: 'loading',
    },
    setHiddenAssetIds: () => {},
    hideAsset: () => {},
    showAsset: () => {},
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
                    // Handle any error that occurred during the unhide process
                    toast.error('Failed to unhide asset.');
                    // Restore the asset ID back to the hidden asset IDs list
                    setHiddenAssetIds([...hiddenAssetIds, assetId]);
                    await set(HIDDEN_ASSET_IDS, hiddenAssetIds);
                }
            };

            const showAssetHiddenToast = async (objectId: string) => {
                toast.success(
                    (t) => (
                        <MovedAssetNotification
                            t={t}
                            destination="Hidden Assets"
                            onUndo={() => undoHideAsset(objectId)}
                        />
                    ),
                    {
                        duration: 4000,
                    },
                );
            };
            showAssetHiddenToast(newAssetId);
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
                toast.error('Failed to show asset.');
                // Restore the asset ID back to the hidden asset IDs list
                setHiddenAssetIds([...hiddenAssetIds, newAssetId]);
                await set(HIDDEN_ASSET_IDS, hiddenAssetIds);
            }

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

            const assetShownToast = async (objectId: string) => {
                toast.success(
                    (t) => (
                        <MovedAssetNotification
                            t={t}
                            destination="Visual Assets"
                            onUndo={() => undoShowAsset(objectId)}
                        />
                    ),
                    {
                        duration: 4000,
                    },
                );
            };

            assetShownToast(newAssetId);
        },
        [hiddenAssetIds],
    );

    const showAsset = (objectId: string) => {
        showAssetId(objectId);
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

interface MovedAssetNotificationProps {
    t: Toast;
    destination: string;
    onUndo: () => void;
}
function MovedAssetNotification({ t, destination, onUndo }: MovedAssetNotificationProps) {
    return (
        <div
            className="flex w-full flex-row items-baseline gap-x-xxs"
            onClick={() => toast.dismiss(t.id)}
        >
            <ButtonUnstyled className="text-body-sm text-neutral-12">
                Moved to {destination}
            </ButtonUnstyled>
            <ButtonUnstyled
                onClick={() => {
                    onUndo();
                    toast.dismiss(t.id);
                }}
                className="ml-auto mr-sm text-body-sm text-neutral-12"
            >
                UNDO
            </ButtonUnstyled>
        </div>
    );
}
