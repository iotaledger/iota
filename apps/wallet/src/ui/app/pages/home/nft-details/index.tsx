// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_app/shared/ButtonUI';
import { Collapsible } from '_app/shared/collapse';
import { Link } from '_app/shared/Link';
import { ExplorerLinkType } from '_components/explorer-link/ExplorerLinkType';
import { LabelValueItem } from '_components/LabelValueItem';
import { LabelValuesContainer } from '_components/LabelValuesContainer';
import Loading from '_components/loading';
import { NFTDisplayCard } from '_components/nft-display';
import { useGetNFTMeta, useNFTBasicData, useOwnedNFT } from '_hooks';
import { useBuyNLargeAsset } from '_src/ui/app/components/buynlarge/useBuyNLargeAsset';
import { useConfig } from '_src/ui/app/components/buynlarge/useConfig';
import { useExplorerLink } from '_src/ui/app/hooks/useExplorerLink';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import PageTitle from '_src/ui/app/shared/PageTitle';
import { Text } from '_src/ui/app/shared/text';
import { ArrowRight16, ArrowUpRight12 } from '@iota/icons';
import { formatAddress } from '@iota/iota.js/utils';
import cl from 'clsx';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';

type NftFields = {
    metadata?: { fields?: { attributes?: { fields?: { keys: string[]; values: string[] } } } };
};

function NFTDetailsPage() {
    const [searchParams] = useSearchParams();
    const nftId = searchParams.get('objectId');
    const { data: objectData, isPending: isNftLoading } = useOwnedNFT(nftId || '');
    const isTransferable =
        !!objectData &&
        objectData.content?.dataType === 'moveObject' &&
        objectData.content?.hasPublicTransfer;
    const { nftFields, fileExtensionType, filePath } = useNFTBasicData(objectData);
    const navigate = useNavigate();
    const buyNLargeConfig = useConfig();
    const { objectType } = useBuyNLargeAsset();

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
    const objectExplorerLink = useExplorerLink({
        type: ExplorerLinkType.object,
        objectID: nftId || '',
    });
    const ownerAddress =
        (objectData?.owner &&
            typeof objectData?.owner === 'object' &&
            'AddressOwner' in objectData.owner &&
            objectData.owner.AddressOwner) ||
        '';
    const ownerExplorerLink = useExplorerLink({
        type: ExplorerLinkType.address,
        address: ownerAddress,
    });
    const isGuardLoading = useUnlockedGuard();
    const isPending = isNftLoading || isPendingDisplay || isGuardLoading;

    const isBuyNLarge = objectData?.type === objectType;

    return (
        <div
            className={cl('flex flex-1 flex-col flex-nowrap gap-5', {
                'items-center': isPending,
            })}
        >
            <Loading loading={isPending}>
                {objectData ? (
                    <>
                        <PageTitle
                            title={isBuyNLarge ? buyNLargeConfig?.sheetTitle : undefined}
                            back
                        />
                        <div className="flex flex-1 flex-col flex-nowrap items-stretch gap-8">
                            <div className="flex flex-col flex-nowrap items-center gap-3 self-center">
                                {isBuyNLarge ? (
                                    <div className="text-center">
                                        <Text color="steel-darker" weight="medium">
                                            {buyNLargeConfig?.sheetDescription}
                                        </Text>
                                    </div>
                                ) : null}
                                <NFTDisplayCard
                                    objectId={nftId!}
                                    size="xl"
                                    borderRadius="xl"
                                    playable
                                />
                                {nftId && !isBuyNLarge ? (
                                    <Link
                                        color="steelDark"
                                        weight="semibold"
                                        size="captionSmall"
                                        href={objectExplorerLink || ''}
                                        text="VIEW ON EXPLORER"
                                        after={<ArrowUpRight12 />}
                                    />
                                ) : null}
                            </div>
                            {!isBuyNLarge ? (
                                <>
                                    <LabelValuesContainer>
                                        {ownerExplorerLink ? (
                                            <LabelValueItem
                                                label="Owner"
                                                value={
                                                    <Link
                                                        color="iotaDark"
                                                        weight="medium"
                                                        size="body"
                                                        mono
                                                        href={ownerExplorerLink}
                                                        text={formatAddress(ownerAddress)}
                                                        title="View on Iota Explorer"
                                                    />
                                                }
                                            />
                                        ) : null}
                                        <LabelValueItem
                                            label="Object Id"
                                            value={
                                                nftId ? (
                                                    <Link
                                                        color="iotaDark"
                                                        weight="medium"
                                                        size="body"
                                                        mono
                                                        href={objectExplorerLink || ''}
                                                        text={formatAddress(nftId)}
                                                        title="View on Iota Explorer"
                                                    />
                                                ) : null
                                            }
                                        />
                                        <LabelValueItem
                                            label="Media Type"
                                            value={
                                                filePath &&
                                                fileExtensionType.name &&
                                                fileExtensionType.type
                                                    ? `${fileExtensionType.name} ${fileExtensionType.type}`
                                                    : '-'
                                            }
                                        />
                                    </LabelValuesContainer>
                                    <Collapsible defaultOpen title="Details">
                                        <LabelValuesContainer>
                                            <LabelValueItem
                                                label="Name"
                                                value={nftDisplayData?.name}
                                            />
                                            <LabelValueItem
                                                label="Description"
                                                value={nftDisplayData?.description}
                                                multiline
                                            />
                                            <LabelValueItem
                                                label="Creator"
                                                value={nftDisplayData?.creator}
                                                parseUrl
                                            />
                                            <LabelValueItem
                                                label="Link"
                                                value={nftDisplayData?.link}
                                                parseUrl
                                            />
                                            <LabelValueItem
                                                label="Website"
                                                value={nftDisplayData?.projectUrl}
                                                parseUrl
                                            />
                                        </LabelValuesContainer>
                                    </Collapsible>
                                    {metaKeys.length ? (
                                        <Collapsible title="Attributes" defaultOpen>
                                            <LabelValuesContainer>
                                                {metaKeys.map((aKey, idx) => (
                                                    <LabelValueItem
                                                        key={aKey}
                                                        label={aKey}
                                                        value={
                                                            typeof metaValues[idx] === 'object'
                                                                ? JSON.stringify(metaValues[idx])
                                                                : metaValues[idx]
                                                        }
                                                    />
                                                ))}
                                            </LabelValuesContainer>
                                        </Collapsible>
                                    ) : null}
                                    <div className="mb-3 flex flex-1 items-end">
                                        <Button
                                            variant="primary"
                                            size="tall"
                                            disabled={!isTransferable}
                                            to={`/nft-transfer/${nftId}`}
                                            title={
                                                isTransferable
                                                    ? undefined
                                                    : "Unable to send. NFT doesn't have public transfer method"
                                            }
                                            text="Send NFT"
                                            after={<ArrowRight16 />}
                                        />
                                    </div>
                                </>
                            ) : (
                                <Button
                                    variant="secondary"
                                    onClick={() => navigate(-1)}
                                    text="Okay"
                                />
                            )}
                        </div>
                    </>
                ) : (
                    <Navigate to="/nfts" replace={true} />
                )}
            </Loading>
        </div>
    );
}

export default NFTDetailsPage;
