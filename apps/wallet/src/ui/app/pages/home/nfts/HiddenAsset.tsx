// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { useNavigate } from 'react-router-dom';
import { useHiddenAssets } from '../assets/HiddenAssetsProvider';
import {
    getKioskIdFromOwnerCap,
    isKioskOwnerToken,
    useGetNFTMeta,
    useGetObject,
    useKioskClient,
} from '@iota/core';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageShape,
    ImageType,
} from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useResolveVideo } from '_src/ui/app/hooks/useResolveVideo';
import { VisibilityOff } from '@iota/ui-icons';

export interface HiddenAssetProps {
    data: IotaObjectData | null | undefined;
    display:
        | {
              [key: string]: string;
          }
        | null
        | undefined;
}

export default function HiddenAsset(item: HiddenAssetProps) {
    const { showAsset } = useHiddenAssets();
    const kioskClient = useKioskClient();
    const navigate = useNavigate();
    const { objectId, type } = item.data!;
    const { data: objectData } = useGetObject(objectId);
    const { data: nftMeta } = useGetNFTMeta(objectId);

    const nftName = nftMeta?.name || formatAddress(objectId);
    const nftImageUrl = nftMeta?.imageUrl || '';
    const video = useResolveVideo(objectData);
    return (
        <ErrorBoundary>
            <Card
                type={CardType.Default}
                onClick={() => {
                    navigate(
                        isKioskOwnerToken(kioskClient.network, item.data)
                            ? `/kiosk?${new URLSearchParams({
                                  kioskId: getKioskIdFromOwnerCap(item.data!),
                              })}`
                            : `/nft-details?${new URLSearchParams({
                                  objectId,
                              }).toString()}`,
                    );
                    ampli.clickedCollectibleCard({
                        objectId,
                        collectibleType: type!,
                    });
                }}
            >
                <CardImage type={ImageType.BgTransparent} shape={ImageShape.SquareRounded}>
                    {video ? (
                        <video
                            className="h-full w-full object-cover"
                            src={video}
                            controls
                            autoPlay
                            muted
                        />
                    ) : (
                        <img
                            src={nftImageUrl}
                            alt={nftName}
                            className="h-full w-full object-cover"
                        />
                    )}
                </CardImage>
                <CardBody title={nftMeta?.name ?? 'Asset'} subtitle={formatAddress(objectId)} />
                <CardAction
                    type={CardActionType.Link}
                    onClick={() => {
                        showAsset(objectId);
                    }}
                    icon={<VisibilityOff />}
                />
            </Card>
        </ErrorBoundary>
    );
}
