// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { ExplorerLinkType, useNftDetails, Collapsible } from '@iota/core';
import {
    Button,
    ButtonType,
    Header,
    KeyValueInfo,
    VisualAssetCard,
    VisualAssetType,
} from '@iota/apps-ui-kit';
import Link from 'next/link';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Layout, LayoutBody, LayoutFooter } from '../../Staking/views/Layout';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { ExplorerLink } from '@/components/ExplorerLink';
import { useCurrentAccount } from '@iota/dapp-kit';

interface DetailsViewProps {
    asset: IotaObjectData;
    handleClose: () => void;
    handleSend: () => void;
}

export function DetailsView({ handleClose, asset, handleSend }: DetailsViewProps) {
    const account = useCurrentAccount();

    const senderAddress = account?.address ?? '';
    const objectId = asset.objectId;

    const {
        nftName,
        nftImageUrl,
        nftMeta,
        ownerAddress,
        isTransferable,
        metaKeys,
        metaValues,
        formatMetaValue,
        isContainedInKiosk,
        kioskItem,
    } = useNftDetails(objectId, senderAddress);

    function handleMoreAboutKiosk() {
        window.open('https://wiki.iota.org/', '_blank');
    }

    function handleMarketplace() {
        window.open('https://wiki.iota.org/', '_blank');
    }

    return (
        <Layout>
            <Header title="Asset" onClose={handleClose} titleCentered />
            <LayoutBody>
                <div className="flex w-full flex-col items-center justify-center gap-xs">
                    <div className="w-[172px]">
                        <VisualAssetCard
                            assetSrc={nftImageUrl}
                            assetTitle={nftName}
                            assetType={VisualAssetType.Image}
                            altText={nftName || 'NFT'}
                            isHoverable={false}
                        />
                    </div>
                    <ExplorerLink type={ExplorerLinkType.Object} objectID={objectId}>
                        <Button type={ButtonType.Ghost} text="View on Explorer" />
                    </ExplorerLink>
                    <div className="flex w-full flex-col gap-md">
                        <div className="flex flex-col gap-xxxs">
                            <span className="text-title-lg text-neutral-10 dark:text-neutral-92">
                                {nftMeta?.name}
                            </span>
                            {nftMeta?.description ? (
                                <span className="text-body-md text-neutral-60">
                                    {nftMeta?.description}
                                </span>
                            ) : null}
                        </div>

                        {(nftMeta?.projectUrl || !!nftMeta?.creator) && (
                            <div className="flex flex-col gap-xs">
                                {nftMeta?.projectUrl && (
                                    <KeyValueInfo
                                        keyText="Website"
                                        value={
                                            <Link href={nftMeta?.projectUrl}>
                                                {nftMeta?.projectUrl}
                                            </Link>
                                        }
                                        fullwidth
                                    />
                                )}
                                {nftMeta?.creator && (
                                    <KeyValueInfo
                                        keyText="Creator"
                                        value={nftMeta?.creator ?? '-'}
                                        fullwidth
                                    />
                                )}
                            </div>
                        )}

                        <Collapsible defaultOpen title="Details">
                            <div className="flex flex-col gap-xs px-md pb-xs pt-sm">
                                {ownerAddress && (
                                    <KeyValueInfo
                                        keyText="Owner"
                                        value={
                                            <ExplorerLink
                                                type={ExplorerLinkType.Address}
                                                address={ownerAddress}
                                            >
                                                {formatAddress(ownerAddress)}
                                            </ExplorerLink>
                                        }
                                        fullwidth
                                    />
                                )}
                                {objectId && (
                                    <KeyValueInfo
                                        keyText="Object ID"
                                        value={formatAddress(objectId)}
                                        fullwidth
                                    />
                                )}
                            </div>
                        </Collapsible>
                        {metaKeys.length ? (
                            <Collapsible defaultOpen title="Attributes">
                                <div className="flex flex-col gap-xs px-md pb-xs pt-sm">
                                    {metaKeys.map((aKey, idx) => {
                                        const { value, valueLink } = formatMetaValue(
                                            metaValues[idx],
                                        );
                                        return (
                                            <KeyValueInfo
                                                key={idx}
                                                keyText={aKey}
                                                value={
                                                    valueLink ? (
                                                        <Link key={aKey} href={valueLink || ''}>
                                                            {value}
                                                        </Link>
                                                    ) : (
                                                        value
                                                    )
                                                }
                                                fullwidth
                                            />
                                        );
                                    })}
                                </div>
                            </Collapsible>
                        ) : null}
                    </div>
                </div>
            </LayoutBody>
            <LayoutFooter>
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
                            disabled={!isTransferable}
                            onClick={handleSend}
                            text="Send"
                            fullWidth
                        />
                    )}
                </div>
            </LayoutFooter>
        </Layout>
    );
}
