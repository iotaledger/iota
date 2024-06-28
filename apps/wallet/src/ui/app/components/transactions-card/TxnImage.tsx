// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetNFTMeta } from '@iota/core';
import { Text } from '_app/shared/text';
import { NftImage } from '_components/nft-display/NftImage';
import { cx } from 'class-variance-authority';

//TODO merge all NFT image displays
export function TxnImage({ id, actionLabel }: { id: string; actionLabel?: string }) {
    const { data: nftMeta } = useGetNFTMeta(id);

    return nftMeta?.imageUrl ? (
        <div className={cx(actionLabel ? 'flex flex-col gap-2 py-3.5 first:pt-0' : '')}>
            {actionLabel ? (
                <Text variant="body" weight="medium" color="steel-darker">
                    {actionLabel}
                </Text>
            ) : null}
            <div className="flex w-full gap-2">
                <NftImage borderRadius="sm" size="xs" name={nftMeta.name} src={nftMeta.imageUrl} />
                <div className="flex w-56 flex-col justify-center gap-1 break-all">
                    {nftMeta.name && (
                        <Text color="gray-90" weight="semibold" variant="subtitleSmall" truncate>
                            {nftMeta.name}
                        </Text>
                    )}
                    {nftMeta.description && (
                        <Text color="steel-darker" weight="medium" variant="subtitleSmall" truncate>
                            {nftMeta.description}
                        </Text>
                    )}
                </div>
            </div>
        </div>
    ) : null;
}
