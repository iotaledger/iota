// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { useGetKioskContents, getKioskIdFromOwnerCap, useNftDetails } from '@iota/core';
import { Header, LoadingIndicator, VisualAssetCard, VisualAssetType } from '@iota/apps-ui-kit';
import { DialogLayoutBody } from '../../layout';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useCurrentAccount } from '@iota/dapp-kit';

interface DetailsViewProps {
    asset: IotaObjectData;
    onClose: () => void;
}

export function KioskDetailsView({ onClose, asset }: DetailsViewProps) {
    const account = useCurrentAccount();
    const senderAddress = account?.address ?? '';
    const objectId = getKioskIdFromOwnerCap(asset);
    const { data: kioskData, isPending } = useGetKioskContents(account?.address);
    const kiosk = kioskData?.kiosks.get(objectId);
    const items = kiosk?.items;

    if (isPending) {
        return (
            <div className="flex h-full items-center justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <>
            <Header title="Kiosk" onClose={onClose} titleCentered />
            <DialogLayoutBody>
                {items?.map((item) => {
                    return item.data?.objectId ? (
                        <KioskItem
                            key={item.data?.objectId}
                            object={item.data}
                            address={senderAddress}
                        />
                    ) : null;
                })}
            </DialogLayoutBody>
            {/* <DialogLayoutFooter>
                <div className="flex flex-col">
                    {isContainedInKiosk && kioskItem?.isLocked ? (
                        <div className="flex flex-col gap-2">
                            <Button
                                type={ButtonType.Secondary}
                                onClick={handleMoreAboutKiosk}
                                text="Learn more about Kiosks"
                            />
                            <Button
                                type={ButtonType.Primary}
                                onClick={handleMarketplace}
                                text="Marketplace"
                            />
                        </div>
                    ) : (
                        <Button
                            disabled={!isAssetTransferable}
                            onClick={onSend}
                            text="Send"
                            fullWidth
                        />
                    )}
                </div>
            </DialogLayoutFooter> */}
        </>
    );
}

interface KioskItemProps {
    object: IotaObjectData;
    address: string;
}

function KioskItem({ object, address }: KioskItemProps) {
    const {
        nftName,
        nftImageUrl,
        // nftDisplayData,
        // ownerAddress,
        // isAssetTransferable,
        // metaKeys,
        // metaValues,
        // formatMetaValue,
        // isContainedInKiosk,
        // kioskItem,
        // objectData,
    } = useNftDetails(object.objectId, address);

    return (
        <VisualAssetCard
            assetSrc={nftImageUrl}
            assetTitle={nftName}
            assetType={VisualAssetType.Image}
            altText={nftName || 'NFT'}
            isHoverable={false}
        />
    );
}
