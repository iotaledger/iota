// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon } from '_app/shared/image-icon';
import { useCoinMetadata } from '@iota/core';
import { Iota, Unstaked } from '@iota/icons';
import { normalizeStructTag, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { cva, type VariantProps } from 'class-variance-authority';

import { useCoinMetadataOverrides } from '../../hooks/useCoinMetadataOverride';

const imageStyle = cva(['rounded-full flex'], {
	variants: {
		size: {
			sm: 'w-6 h-6',
			md: 'w-7.5 h-7.5',
			lg: 'md:w-10 md:h-10 w-8 h-8',
			xl: 'md:w-31.5 md:h-31.5 w-16 h-16 ',
		},
		fill: {
			iota: 'bg-iota',
			iotaPrimary2023: 'bg-iota-primaryBlue2023',
		},
	},
	defaultVariants: {
		size: 'md',
		fill: 'iotaPrimary2023',
	},
});

function IotaCoin() {
	return (
		<Iota className="flex items-center w-full h-full justify-center text-white p-1.5 text-body rounded-full" />
	);
}

type NonIotaCoinProps = {
	coinType: string;
};

function NonIotaCoin({ coinType }: NonIotaCoinProps) {
	const { data: coinMeta } = useCoinMetadata(coinType);
	const coinMetadataOverrides = useCoinMetadataOverrides();

	return (
		<div className="flex h-full w-full items-center justify-center text-white bg-steel rounded-full overflow-hidden">
			{coinMeta?.iconUrl ? (
				<ImageIcon
					src={coinMetadataOverrides[coinType]?.iconUrl ?? coinMeta.iconUrl}
					label={coinMeta.name || coinType}
					fallback={coinMeta.name || coinType}
					rounded="full"
				/>
			) : (
				<Unstaked />
			)}
		</div>
	);
}

export interface CoinIconProps extends VariantProps<typeof imageStyle> {
	coinType: string;
}

export function CoinIcon({ coinType, ...styleProps }: CoinIconProps) {
	const isIota = coinType
		? normalizeStructTag(coinType) === normalizeStructTag(IOTA_TYPE_ARG)
		: false;

	return (
		<div className={imageStyle(styleProps)}>
			{isIota ? <IotaCoin /> : <NonIotaCoin coinType={coinType} />}
		</div>
	);
}
