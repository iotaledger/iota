// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';
import { Check12 } from '@iota/icons';
import { toast } from 'react-hot-toast';

import { Link as InlineLink } from '../../shared/Link';

export const showAssetHiddenToast = (assetId: string, undoHideAsset: (assetId: string) => void) => {
    toast.custom(
        (t) => (
            <div
                className="flex w-full items-center justify-between gap-2 rounded-full border-solid border-gray-45 bg-white px-3 py-2 shadow-notification"
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
                            to="/nfts/hidden-assets"
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
                            undoHideAsset(assetId);
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

export const showAssetShownToast = (assetId: string, undoShowAsset: (assetId: string) => void) => {
    toast.custom(
        (t) => (
            <div
                className="flex w-full items-center justify-between gap-2 rounded-full border-solid border-gray-45 bg-white px-3 py-2 shadow-notification"
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
                            undoShowAsset(assetId);
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
