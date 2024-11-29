// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    useGetNFTMeta,
    useOwnedNFT,
    useNFTBasicData,
    useGetKioskContents,
    useIsAssetTransferable,
} from './';
import { formatAddress } from '@iota/iota-sdk/utils';
import { truncateString } from '../utils';

type NftField = { keys: string[]; values: string[] };

type NftFields = {
    metadata?: { fields?: { attributes?: { fields?: NftField } } };
};

export function useNftDetails(nftId: string, accountAddress: string | null) {
    const { data: objectData, isPending: isNftLoading } = useOwnedNFT(nftId || '', accountAddress);
    const { data } = useGetKioskContents(accountAddress);

    const isContainedInKiosk = data?.lookup.get(nftId!);
    const kioskItem = data?.list.find((k) => k.data?.objectId === nftId);

    const { data: isTransferable } = useIsAssetTransferable(objectData);

    const { nftFields } = useNFTBasicData(objectData);

    const { data: nftMeta, isPending: isPendingMeta } = useGetNFTMeta(nftId);

    const nftName = nftMeta?.name || formatAddress(nftId);
    const nftImageUrl = nftMeta?.imageUrl || '';

    // Extract either the attributes, or use the top-level NFT fields:
    const { keys: metaKeys, values: metaValues } =
        (nftFields as NftFields)?.metadata?.fields?.attributes?.fields ||
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
