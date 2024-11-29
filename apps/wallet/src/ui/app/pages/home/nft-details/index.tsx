// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { ExplorerLink, ExplorerLinkType, Loading, NFTDisplayCard, PageTemplate } from '_components';
import { useNFTBasicData, useOwnedNFT } from '_hooks';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import { useIsAssetTransferable, useGetKioskContents, useGetNFTMeta, Collapsible } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import cl from 'clsx';
import { Link, Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { Button, ButtonType, KeyValueInfo } from '@iota/apps-ui-kit';
import { truncateString } from '_src/ui/app/helpers';

type NftFields = {
    metadata?: { fields?: { attributes?: { fields?: { keys: string[]; values: string[] } } } };
};

function NFTDetailsPage() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const nftId = searchParams.get('objectId');
    const accountAddress = useActiveAddress();
    const { data: objectData, isPending: isNftLoading } = useOwnedNFT(nftId || '', accountAddress);
    const { data: isAssetTransferable, isLoading: isCheckingAssetTransferability } =
        useIsAssetTransferable(objectData);
    const { nftFields, fileExtensionType, filePath } = useNFTBasicData(objectData);
    const address = useActiveAddress();
    const { data } = useGetKioskContents(address);

    const isContainedInKiosk = data?.lookup.get(nftId!);
    const kioskItem = data?.list.find((k) => k.data?.objectId === nftId);

    // Extract either the attributes, or use the top-level NFT fields:
    const metaFields =
        (nftFields as NftFields)?.metadata?.fields?.attributes?.fields ||
        Object.entries(nftFields ?? {})
            .filter(([key]) => key !== 'id')
            .reduce(
                (acc, [key, value]) => {
                    acc.keys.push(key);
                    acc.values.push(value as string);
                    return acc;
                },
                { keys: [] as string[], values: [] as string[] },
            );
    const metaKeys: string[] = metaFields ? metaFields.keys : [];
    const metaValues = metaFields ? metaFields.values : [];
    const { data: nftDisplayData, isPending: isPendingDisplay } = useGetNFTMeta(nftId || '');
    const ownerAddress =
        (objectData?.owner &&
            typeof objectData?.owner === 'object' &&
            'AddressOwner' in objectData.owner &&
            objectData.owner.AddressOwner) ||
        '';
    const isGuardLoading = useUnlockedGuard();
    const isPending =
        isNftLoading || isPendingDisplay || isGuardLoading || isCheckingAssetTransferability;

    function handleMoreAboutKiosk() {
        window.open('https://docs.iota.org/references/ts-sdk/kiosk/', '_blank');
    }

    function handleMarketplace() {
        // TODO: https://github.com/iotaledger/iota/issues/4024
        window.open('https://docs.iota.org/references/ts-sdk/kiosk/', '_blank');
    }

    function handleSend() {
        navigate(`/nft-transfer/${nftId}`);
    }

    function formatMetaValue(value: string | object) {
        if (typeof value === 'object') {
            return {
                value: JSON.stringify(value),
                valueLink: undefined,
            };
        } else {
            if (value.includes('http')) {
                return {
                    value: value.startsWith('http')
                        ? truncateString(value, 20, 8)
                        : formatAddress(value),
                    valueLink: value,
                };
            }
            return {
                value: value,
                valueLink: undefined,
            };
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
                                            <span className="text-title-lg text-neutral-10 dark:text-neutral-92">
                                                {nftDisplayData?.name}
                                            </span>
                                            {nftDisplayData?.description ? (
                                                <span className="text-body-md text-neutral-60">
                                                    {nftDisplayData?.description}
                                                </span>
                                            ) : null}
                                        </div>
                                        {nftDisplayData?.projectUrl ||
                                            (nftDisplayData?.creator && (
                                                <div className="flex flex-col gap-xs">
                                                    {nftDisplayData?.projectUrl && (
                                                        <KeyValueInfo
                                                            keyText="Website"
                                                            value={
                                                                <Link
                                                                    to={nftDisplayData?.projectUrl}
                                                                >
                                                                    {nftDisplayData?.projectUrl}
                                                                </Link>
                                                            }
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
                                            ))}
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
                                                            >
                                                                {formatAddress(ownerAddress)}
                                                            </ExplorerLink>
                                                        }
                                                        fullwidth
                                                    />
                                                )}
                                                {nftId && (
                                                    <KeyValueInfo
                                                        keyText="Object ID"
                                                        value={formatAddress(nftId)}
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
                                                        const { value, valueLink } =
                                                            formatMetaValue(metaValues[idx]);
                                                        return (
                                                            <KeyValueInfo
                                                                key={idx}
                                                                keyText={aKey}
                                                                value={
                                                                    <Link
                                                                        key={aKey}
                                                                        to={valueLink || ''}
                                                                    >
                                                                        {value}
                                                                    </Link>
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

export default NFTDetailsPage;
