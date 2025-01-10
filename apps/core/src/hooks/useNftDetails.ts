// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    useGetNFTDisplay,
    useOwnedNFT,
    useNFTBasicData,
    useGetKioskContents,
    useIsAssetTransferable,
} from './';
import { formatAddress } from '@iota/iota-sdk/utils';

type NftField = { keys: string[]; values: string[] };

type NftFields = {
    metadata?: { fields?: { attributes?: { fields?: NftField } } };
};

export function useNftDetails(nftId: string, accountAddress: string | null) {
    const { data: objectData, isPending: isNftLoading } = useOwnedNFT(nftId || '', accountAddress);
    const { data } = useGetKioskContents(accountAddress);

    const isContainedInKiosk = data?.lookup.get(nftId!);
    const kioskItem = data?.list.find((k) => k.data?.objectId === nftId);

    const { data: isAssetTransferable, isLoading: isCheckingAssetTransferability } =
        useIsAssetTransferable(objectData);

    const { nftFields } = useNFTBasicData(objectData);

    const { data: nftDisplayData, isPending: isPendingNftDislpay } = useGetNFTDisplay(nftId);

    const nftName = nftDisplayData?.name || formatAddress(nftId);
    const nftImageUrl = nftDisplayData?.imageUrl || '';

    // Extract either the attributes, or use the top-level NFT fields:
    const { keys: metaKeys, values: metaValues } =
        (nftFields as unknown as NftFields)?.metadata?.fields?.attributes?.fields ||
        Object.entries(nftFields ?? {})
            .filter(([key]) => key !== 'id')
            .reduce<NftField>(
                (acc, [key, value]) => {
                    acc.keys.push(key);
                    acc.values.push(value as string);
                    return acc;
                },
                { keys: [], values: [] },
            );

    const ownerAddress =
        (objectData?.owner &&
            typeof objectData?.owner === 'object' &&
            'AddressOwner' in objectData.owner &&
            objectData.owner.AddressOwner) ||
        '';

    const isLoading = isNftLoading || isCheckingAssetTransferability || isPendingNftDislpay;

    return {
        isLoading,
        objectData,
        isNftLoading,
        nftName,
        nftImageUrl,
        ownerAddress,
        isCheckingAssetTransferability,
        isAssetTransferable,
        metaKeys,
        metaValues,
        isContainedInKiosk,
        kioskItem,
        nftDisplayData,
        isPendingNftDislpay,
    };
}
