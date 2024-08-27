// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary, NFTDisplayCard } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { EyeClose16 } from '@iota/icons';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { Link } from 'react-router-dom';

import { useHiddenAssets } from '../assets/HiddenAssetsProvider';
import { getKioskIdFromOwnerCap, isKioskOwnerToken, useKioskClient } from '@iota/core';

interface VisualAssetsProps {
    items: IotaObjectData[];
}

export default function VisualAssets({ items }: VisualAssetsProps) {
    const { hideAsset } = useHiddenAssets();
    const kioskClient = useKioskClient();

    return (
        <div className="grid w-full grid-cols-2 gap-x-3.5 gap-y-4">
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
                    <div className="group">
                        <div className="text-gray-60 pointer-events-auto absolute z-10 h-full w-full justify-center p-0 transition-colors duration-200">
                            {!isKioskOwnerToken(kioskClient.network, object) ? (
                                <div className="absolute right-3 top-2 h-8 w-8 rounded-md opacity-0 group-hover:opacity-100">
                                    <Button
                                        variant="hidden"
                                        size="icon"
                                        onClick={(event) => {
                                            event.preventDefault();
                                            event.stopPropagation();
                                            ampli.clickedHideAsset({
                                                objectId: object.objectId,
                                                collectibleType: object.type!,
                                            });
                                            hideAsset(object.objectId);
                                        }}
                                        after={<EyeClose16 />}
                                    />
                                </div>
                            ) : null}
                        </div>
                        <ErrorBoundary>
                            <NFTDisplayCard objectId={object.objectId} isHoverable />
                        </ErrorBoundary>
                    </div>
                </Link>
            ))}
        </div>
    );
}
