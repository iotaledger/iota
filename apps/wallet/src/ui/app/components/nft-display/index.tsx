// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { Heading } from '_app/shared/heading';
import Loading from '_components/loading';
import { NftImage, type NftImageProps } from '_components/nft-display/NftImage';
import { useFileExtensionType, useGetNFTMeta } from '_hooks';
import { isKioskOwnerToken, useGetObject } from '@mysten/core';
import { useKioskClient } from '@mysten/core/src/hooks/useKioskClient';
import { formatAddress } from '@mysten/sui.js/utils';
import { cva } from 'class-variance-authority';
import type { VariantProps } from 'class-variance-authority';

import { useResolveVideo } from '../../hooks/useResolveVideo';
import { Text } from '../../shared/text';
import { Kiosk } from './Kiosk';

const nftDisplayCardStyles = cva('flex flex-nowrap items-center h-full relative', {
	variants: {
		animateHover: {
			true: 'group',
		},
		wideView: {
			true: 'bg-gray-40 p-2.5 rounded-lg gap-2.5 flex-row-reverse justify-between',
			false: '',
		},
		orientation: {
			horizontal: 'flex truncate',
			vertical: 'flex-col',
		},
	},
	defaultVariants: {
		wideView: false,
		orientation: 'vertical',
	},
});

export interface NFTDisplayCardProps extends VariantProps<typeof nftDisplayCardStyles> {
	objectId: string;
	hideLabel?: boolean;
	size: NftImageProps['size'];
	borderRadius?: NftImageProps['borderRadius'];
	playable?: boolean;
	isLocked?: boolean;
}

export function NFTDisplayCard({
	objectId,
	hideLabel,
	size,
	wideView,
	animateHover,
	borderRadius = 'md',
	playable,
	orientation,
	isLocked,
}: NFTDisplayCardProps) {
	const { data: objectData } = useGetObject(objectId);
	const { data: nftMeta, isPending } = useGetNFTMeta(objectId);
	const nftName = nftMeta?.name || formatAddress(objectId);
	const nftImageUrl = nftMeta?.imageUrl || '';
	const video = useResolveVideo(objectData);
	const fileExtensionType = useFileExtensionType(nftImageUrl);
	const kioskClient = useKioskClient();
	const isOwnerToken = isKioskOwnerToken(kioskClient.network, objectData);
	const shouldShowLabel = !wideView && orientation !== 'horizontal';

	return (
		<div className={nftDisplayCardStyles({ animateHover, wideView, orientation })}>
			<Loading loading={isPending}>
				{objectData?.data && isOwnerToken ? (
					<Kiosk
						object={objectData}
						borderRadius={borderRadius}
						size={size}
						orientation={orientation}
						playable={playable}
						showLabel={shouldShowLabel}
					/>
				) : (
					<NftImage
						name={nftName}
						src={nftImageUrl}
						animateHover={animateHover}
						showLabel={shouldShowLabel}
						borderRadius={borderRadius}
						size={size}
						isLocked={isLocked}
						video={video}
					/>
				)}
				{wideView && (
					<div className="ml-1 flex min-w-0 flex-1 flex-col gap-1">
						<Heading variant="heading6" color="gray-90" truncate>
							{nftName}
						</Heading>
						<div className="text-body font-medium text-gray-75">
							{nftImageUrl ? (
								`${fileExtensionType.name} ${fileExtensionType.type}`
							) : (
								<span className="text-bodySmall font-normal uppercase">NO MEDIA</span>
							)}
						</div>
					</div>
				)}

				{orientation === 'horizontal' ? (
					<div className="ml-2 max-w-full flex-1 overflow-hidden text-steel-dark">{nftName}</div>
				) : !isOwnerToken && !hideLabel ? (
					<div className="absolute bottom-2 left-1/2 flex w-10/12 -translate-x-1/2 items-center justify-center rounded-lg bg-white/90 opacity-0 group-hover:opacity-100">
						<div className="mt-0.5 overflow-hidden px-2 py-1">
							<Text variant="subtitleSmall" weight="semibold" mono color="steel-darker" truncate>
								{nftName}
							</Text>
						</div>
					</div>
				) : null}
			</Loading>
		</div>
	);
}
