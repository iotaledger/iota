// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress, useUnlockedGuard, useShouldOpenInNewTab } from '_hooks';
import { ExplorerLink, ExplorerLinkType, Loading, NFTDisplayCard, PageTemplate } from '_components';
import { useNFTBasicData, useNftDetails, Collapsible } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import cl from 'clsx';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { Button, ButtonType, KeyValueInfo } from '@iota/apps-ui-kit';
import { ampli } from '_src/shared/analytics/ampli';
import { openInNewTab } from '_src/shared/utils';

export function NFTDetailsPage() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const nftId = searchParams.get('objectId');
    const accountAddress = useActiveAddress();
    const shouldOpenNewTab = useShouldOpenInNewTab();
    const {
        nftDisplayData,
        isLoading,
        ownerAddress,
        objectData,
        metaKeys,
        metaValues,
        isContainedInKiosk,
        kioskItem,
        isAssetTransferable,
    } = useNftDetails(nftId || '', accountAddress);
    const { fileExtensionType, filePath } = useNFTBasicData(objectData);

    const isGuardLoading = useUnlockedGuard();
    const isPending = isLoading || isGuardLoading;

    function handleMoreAboutKiosk() {
        const url = 'https://docs.iota.org/developer/ts-sdk/kiosk/';
        ampli.openedLink({ type: 'ts-sdk documentation' });
        window.open(url, '_blank', 'noopener noreferrer');
    }

    function handleMarketplace() {
        // TODO: https://github.com/iotaledger/iota/issues/4024
        const url = 'https://docs.iota.org/developer/ts-sdk/kiosk/';
        ampli.openedLink({ type: 'ts-sdk documentation' });
        window.open(url, '_blank', 'noopener noreferrer');
    }

    async function handleSend() {
        const destination = `/nft-transfer/${nftId}`;
        if (shouldOpenNewTab) {
            openInNewTab(destination);
        } else {
            navigate(destination);
        }
    }

    return (
        <PageTemplate
            title="Visual Asset"
            isTitleCentered
            onClose={() => navigate(-1)}
            showBackButton
        >
            <div
                className={cl('flex h-full flex-1 flex-col flex-nowrap gap-5', {
                    'items-center': isPending,
                })}
            >
                <Loading loading={isPending}>
                    {objectData ? (
                        <>
                            <div className="flex h-full flex-1 flex-col flex-nowrap items-stretch gap-lg">
                                <div className="flex h-full flex-col gap-lg overflow-y-auto">
                                    <div className="flex w-[172px] flex-col items-center gap-xs self-center">
                                        <NFTDisplayCard objectId={nftId!} />
                                        {nftId ? (
                                            <ExplorerLink
                                                objectID={nftId}
                                                type={ExplorerLinkType.Object}
                                                eventType="object"
                                            >
                                                <Button
                                                    type={ButtonType.Ghost}
                                                    text="View on Explorer"
                                                />
                                            </ExplorerLink>
                                        ) : null}
                                    </div>
                                    <div className="flex flex-col gap-md">
                                        <div className="flex flex-col gap-xxxs">
                                            <span className="break-words text-title-lg text-iota-neutral-10 dark:text-iota-neutral-92">
                                                {nftDisplayData?.name}
                                            </span>
                                            {nftDisplayData?.description ? (
                                                <span className="break-words text-body-md text-iota-neutral-60">
                                                    {nftDisplayData?.description}
                                                </span>
                                            ) : null}
                                        </div>
                                        {(nftDisplayData?.projectUrl ||
                                            nftDisplayData?.creator) && (
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
                                    </div>
                                    <div className="flex flex-col gap-md">
                                        <Collapsible defaultOpen title="Details">
                                            <div className="flex flex-col gap-xs px-md pb-xs pt-sm">
                                                {ownerAddress && (
                                                    <KeyValueInfo
                                                        keyText="Owner"
                                                        value={
                                                            <ExplorerLink
                                                                type={ExplorerLinkType.Address}
                                                                address={ownerAddress}
                                                                eventType="address"
                                                            >
                                                                <span data-amp-mask>
                                                                    {formatAddress(ownerAddress)}
                                                                </span>
                                                            </ExplorerLink>
                                                        }
                                                        fullwidth
                                                    />
                                                )}
                                                {nftId && (
                                                    <KeyValueInfo
                                                        keyText="Object ID"
                                                        value={
                                                            <span data-amp-mask>
                                                                {formatAddress(nftId)}
                                                            </span>
                                                        }
                                                        fullwidth
                                                    />
                                                )}
                                                <KeyValueInfo
                                                    keyText="Media Type"
                                                    value={
                                                        filePath &&
                                                        fileExtensionType.name &&
                                                        fileExtensionType.type
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
                                                                    typeof metaValues[idx] ===
                                                                    'object'
                                                                        ? JSON.stringify(
                                                                              metaValues[idx],
                                                                          )
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
                                        <div className="flex flex-1 items-end">
                                            <Button
                                                disabled={!isAssetTransferable}
                                                onClick={handleSend}
                                                text="Send"
                                                fullWidth
                                            />
                                        </div>
                                    )}
                                </div>
                            </div>
                        </>
                    ) : (
                        <Navigate to="/nfts" replace={true} />
                    )}
                </Loading>
            </div>
        </PageTemplate>
    );
}
