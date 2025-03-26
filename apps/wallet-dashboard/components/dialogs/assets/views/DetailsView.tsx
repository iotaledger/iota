// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ExplorerLinkType,
    useNftDetails,
    Collapsible,
    useNFTBasicData,
    NftImage,
} from '@iota/core';
import { Button, ButtonType, Header, KeyValueInfo } from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import { DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { ExplorerLink } from '@/components/ExplorerLink';
import { useCurrentAccount } from '@iota/dapp-kit';

interface DetailsViewProps {
    asset: IotaObjectData;
    onClose: () => void;
    onSend: () => void;
    onBack?: () => void;
}

export function DetailsView({ onClose, asset, onSend, onBack }: DetailsViewProps) {
    const account = useCurrentAccount();

    const senderAddress = account?.address ?? '';
    const objectId = asset.objectId;

    const {
        nftName,
        nftImageUrl,
        nftDisplayData,
        ownerAddress,
        isAssetTransferable,
        metaKeys,
        metaValues,
        isContainedInKiosk,
        kioskItem,
        objectData,
    } = useNftDetails(objectId, senderAddress);
    const { fileExtensionType, filePath } = useNFTBasicData(objectData);

    function handleMoreAboutKiosk() {
        window.open('https://docs.iota.org/ts-sdk/kiosk/', '_blank', 'noopener noreferrer');
    }

    function handleMarketplace() {
        // TODO: https://github.com/iotaledger/iota/issues/4024
        window.open('https://docs.iota.org/ts-sdk/kiosk/', '_blank', 'noopener noreferrer');
    }

    return (
        <>
            <Header title="Asset" onClose={onClose} titleCentered onBack={onBack} />
            <DialogLayoutBody>
                <div className="flex w-full flex-col items-center justify-center gap-xs">
                    <div className="w-[172px]">
                        <NftImage src={nftImageUrl} title={nftName || 'NFT'} isHoverable={false} />
                    </div>
                    <ExplorerLink type={ExplorerLinkType.Object} objectID={objectId}>
                        <Button type={ButtonType.Ghost} text="View on Explorer" />
                    </ExplorerLink>
                    <div className="flex w-full flex-col gap-md">
                        <div className="flex flex-col gap-xxxs">
                            <span className="text-title-lg text-neutral-10 dark:text-neutral-92">
                                {nftDisplayData?.name}
                            </span>
                            {nftDisplayData?.description ? (
                                <span className="text-body-md text-neutral-60">
                                    {nftDisplayData?.description}
                                </span>
                            ) : null}
                        </div>

                        {(nftDisplayData?.projectUrl || !!nftDisplayData?.creator) && (
                            <div className="flex flex-col gap-xs">
                                {nftDisplayData?.projectUrl && (
                                    <KeyValueInfo
                                        keyText="Website"
                                        value={nftDisplayData?.projectUrl}
                                        fullwidth
                                    />
                                )}
                                {nftDisplayData?.creator && (
                                    <KeyValueInfo
                                        keyText="Creator"
                                        value={nftDisplayData?.creator ?? '-'}
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
                                <KeyValueInfo
                                    keyText="Media Type"
                                    value={
                                        filePath && fileExtensionType.name && fileExtensionType.type
                                            ? `${fileExtensionType.name} ${fileExtensionType.type}`
                                            : '-'
                                    }
                                    fullwidth
                                />
                            </div>
                        </Collapsible>
                        {metaKeys.length ? (
                            <Collapsible defaultOpen title="Attributes">
                                <div className="flex flex-col gap-xs px-md pb-xs pt-sm">
                                    {metaKeys.map((aKey, idx) => {
                                        return (
                                            <KeyValueInfo
                                                key={idx}
                                                keyText={aKey}
                                                value={
                                                    typeof metaValues[idx] === 'object'
                                                        ? JSON.stringify(metaValues[idx])
                                                        : metaValues[idx]
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
            </DialogLayoutBody>
            <DialogLayoutFooter>
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
            </DialogLayoutFooter>
        </>
    );
}
