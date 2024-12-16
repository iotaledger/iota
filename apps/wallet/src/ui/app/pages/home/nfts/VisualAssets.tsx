// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, NFTDisplayCard, MovedAssetNotification } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { Link } from 'react-router-dom';
import { toast } from 'react-hot-toast';
import {
    useHiddenAssets,
    getKioskIdFromOwnerCap,
    isKioskOwnerToken,
    useKioskClient,
} from '@iota/core';
import { VisibilityOff } from '@iota/ui-icons';

interface VisualAssetsProps {
    items: IotaObjectData[];
}

export default function VisualAssets({ items }: VisualAssetsProps) {
    const { hideAsset, undoHideAsset } = useHiddenAssets();
    const kioskClient = useKioskClient();

    function handleHideAsset(event: React.MouseEvent<HTMLButtonElement>, object: IotaObjectData) {
        event.preventDefault();
        event.stopPropagation();
        ampli.clickedHideAsset({
            objectId: object.objectId,
            collectibleType: object.type!,
        });
        hideAsset(object.objectId).then(() => {
            toast.success(
                (t) => (
                    <MovedAssetNotification
                        t={t}
                        destination="Hidden Assets"
                        onUndo={() =>
                            undoHideAsset(object.objectId).catch(() => {
                                // Handle any error that occurred during the unhide process
                                toast.error('Failed to unhide asset.');
                            })
                        }
                    />
                ),
                {
                    duration: 4000,
                },
            );
        });
    }

    return (
        <div className="grid w-full grid-cols-2 gap-md">
            {items.map((object) => (
                <Link
                    to={
                        isKioskOwnerToken(kioskClient.network, object)
                            ? `/kiosk?${new URLSearchParams({
                                  kioskId: getKioskIdFromOwnerCap(object),
                              })}`
                            : `/nft-details?${new URLSearchParams({
                                  objectId: object.objectId,
                              }).toString()}`
                    }
                    onClick={() => {
                        ampli.clickedCollectibleCard({
                            objectId: object.objectId,
                            collectibleType: object.type!,
                        });
                    }}
                    key={object.objectId}
                    className="relative no-underline"
                >
                    <ErrorBoundary>
                        <NFTDisplayCard
                            objectId={object.objectId}
                            isHoverable={!isKioskOwnerToken(kioskClient.network, object)}
                            hideLabel
                            icon={<VisibilityOff />}
                            onIconClick={(e) => handleHideAsset(e, object)}
                        />
                    </ErrorBoundary>
                </Link>
            ))}
        </div>
    );
}
