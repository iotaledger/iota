// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useGetNFTMeta, useOwnedNFT, useNFTBasicData, useGetKioskContents } from './';
import { formatAddress } from '@iota/iota-sdk/utils';
import { isAssetTransferable, truncateString } from '../utils';

type NftFields = {
    metadata?: { fields?: { attributes?: { fields?: { keys: string[]; values: string[] } } } };
};

export function useNftDetails(nftId: string, accountAddress: string | null) {
    const { data: objectData, isPending: isNftLoading } = useOwnedNFT(nftId || '', accountAddress);
    const { data } = useGetKioskContents(accountAddress);

    const isContainedInKiosk = data?.lookup.get(nftId!);
    const kioskItem = data?.list.find((k) => k.data?.objectId === nftId);

    const isTransferable = isAssetTransferable(objectData);

    const { nftFields } = useNFTBasicData(objectData);

    const { data: nftMeta, isPending: isPendingMeta } = useGetNFTMeta(nftId);

    const nftName = nftMeta?.name || formatAddress(nftId);
    const nftImageUrl = nftMeta?.imageUrl || '';

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

    const ownerAddress =
        (objectData?.owner &&
            typeof objectData?.owner === 'object' &&
            'AddressOwner' in objectData.owner &&
            objectData.owner.AddressOwner) ||
        '';

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

    return {
        objectData,
        isNftLoading,
        nftName,
        nftImageUrl,
        ownerAddress,
        isTransferable,
        metaKeys,
        metaValues,
        formatMetaValue,

        isContainedInKiosk,
        kioskItem,

        nftMeta,
        isPendingMeta,
    };
}
