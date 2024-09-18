// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';
import { Check12 } from '@iota/icons';
import { get, set } from 'idb-keyval';
import {
    createContext,
    useCallback,
    useContext,
    useEffect,
    useState,
    type ReactNode,
    useMemo,
} from 'react';
import { toast } from 'react-hot-toast';
import { Link as InlineLink } from '../../../shared/Link';

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
            const hiddenAssetIds = hiddenAssets.type === 'loaded' ? hiddenAssets.assetIds : [];
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
                toast.custom(
                    (t) => (
                        <div
                            className="border-gray-45 flex w-full items-center justify-between gap-2 rounded-full border-solid bg-white px-3 py-2 shadow-notification"
                            style={{
                                animation: 'fade-in-up 200ms ease-in-out',
                            }}
                        >
                            <div className="flex items-center gap-2">
                                <Check12 className="text-gray-90" />
                                <div
                                    onClick={() => {
                                        toast.dismiss(t.id);
                                    }}
                                >
                                    <InlineLink
                                        to="/nfts"
                                        color="hero"
                                        weight="medium"
                                        before={
                                            <Text variant="body" color="gray-80">
                                                Moved to
                                            </Text>
                                        }
                                        text="Hidden Assets"
                                        onClick={() => toast.dismiss(t.id)}
                                    />
                                </div>
                            </div>

                            <div className="w-auto">
                                <InlineLink
                                    size="bodySmall"
                                    onClick={() => {
                                        undoHideAsset(objectId);
                                        toast.dismiss(t.id);
                                    }}
                                    color="hero"
                                    weight="medium"
                                    text="UNDO"
                                />
                            </div>
                        </div>
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
                toast.custom(
                    (t) => (
                        <div
                            className="border-gray-45 flex w-full items-center justify-between gap-2 rounded-full border-solid bg-white px-3 py-2 shadow-notification"
                            style={{
                                animation: 'fade-in-up 200ms ease-in-out',
                            }}
                        >
                            <div className="flex items-center gap-1">
                                <Check12 className="text-gray-90" />
                                <div
                                    onClick={() => {
                                        toast.dismiss(t.id);
                                    }}
                                >
                                    <InlineLink
                                        to="/nfts"
                                        color="hero"
                                        weight="medium"
                                        before={
                                            <Text variant="body" color="gray-80">
                                                Moved to
                                            </Text>
                                        }
                                        text="Visual Assets"
                                        onClick={() => toast.dismiss(t.id)}
                                    />
                                </div>
                            </div>

                            <div className="w-auto">
                                <InlineLink
                                    size="bodySmall"
                                    onClick={() => {
                                        undoShowAsset(objectId);
                                        toast.dismiss(t.id);
                                    }}
                                    color="hero"
                                    weight="medium"
                                    text="UNDO"
                                />
                            </div>
                        </div>
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
