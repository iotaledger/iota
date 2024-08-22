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

interface HiddenAssetsProps {
    items: {
        data: IotaObjectData | null | undefined;
        display:
            | {
                  [key: string]: string;
              }
            | null
            | undefined;
    }[];
}

export default function HiddenAssets({ items }: HiddenAssetsProps) {
    const { showAsset } = useHiddenAssets();
    const kioskClient = useKioskClient();

    return (
        <div className="grid w-full grid-cols-2 gap-x-3.5 gap-y-4">
            {items?.map((object) => {
                const { objectId, type } = object.data!;
                return (
                    <div className="flex items-center justify-between pr-1 pt-2" key={objectId}>
                        <Link
                            to={
                                isKioskOwnerToken(kioskClient.network, object.data)
                                    ? `/kiosk?${new URLSearchParams({
                                          kioskId: getKioskIdFromOwnerCap(object.data!),
                                      })}`
                                    : `/nft-details?${new URLSearchParams({
                                          objectId,
                                      }).toString()}`
                            }
                            onClick={() => {
                                ampli.clickedCollectibleCard({
                                    objectId,
                                    collectibleType: type!,
                                });
                            }}
                            className="relative truncate no-underline"
                        >
                            <ErrorBoundary>
                                <NFTDisplayCard
                                    objectId={objectId}
                                    size="xs"
                                    orientation="horizontal"
                                />
                            </ErrorBoundary>
                        </Link>
                        <div className="h-8 w-8">
                            <Button
                                variant="secondaryIota"
                                size="icon"
                                onClick={() => {
                                    showAsset(objectId);
                                }}
                                after={<EyeClose16 />}
                            />
                        </div>
                    </div>
                );
            })}
        </div>
    );
}
