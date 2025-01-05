// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, NFTDisplayCard } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { Link } from 'react-router-dom';

import { useHiddenAssets } from '../assets/HiddenAssetsProvider';
import { getKioskIdFromOwnerCap, isKioskOwnerToken, useKioskClient } from '@iota/core';
import { VisibilityOff } from '@iota/ui-icons';

interface VisualAssetsProps {
    items: IotaObjectData[];
}

export function VisualAssets({ items }: VisualAssetsProps) {
    const { hideAsset } = useHiddenAssets();
    const kioskClient = useKioskClient();

    function handleHideAsset(event: React.MouseEvent<HTMLButtonElement>, object: IotaObjectData) {
        event.preventDefault();
        event.stopPropagation();
        ampli.clickedHideAsset({
            objectId: object.objectId,
            collectibleType: object.type!,
        });
        hideAsset(object.objectId);
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
