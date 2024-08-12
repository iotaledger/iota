// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getKioskIdFromOwnerCap, hasDisplayData, useGetKioskContents } from '@iota/core';
import { type IotaObjectResponse } from '@iota/iota.js/client';
import cl from 'clsx';

import { useActiveAddress } from '_app/hooks';
import { Text } from '_app/shared/text';
import { NftImage, type NftImageProps } from './NftImage';

interface KioskProps extends Partial<NftImageProps> {
    object: IotaObjectResponse;
    orientation?: 'vertical' | 'horizontal' | null;
}

// used to prevent the top image from overflowing the bottom of the container
// (clip-path is used instead of overflow-hidden as it can be animated)
const CLIP_PATH = '[clip-path:inset(0_0_7px_0_round_12px)] group-hover:[clip-path:inset(0_0_0_0)]';

const TIMING =
    'transition-all group-hover:delay-[0.25s] duration-300 ease-[cubic-bezier(0.68,-0.55,0.265,1.55)]';
const cardStyles = [
    `scale-100 group-hover:scale-95 object-cover origin-bottom z-30 group-hover:translate-y-0 translate-y-2 group-hover:shadow-md`,
    `scale-[0.95] group-hover:-rotate-6 group-hover:-translate-x-5 group-hover:-translate-y-2 z-20 translate-y-0 group-hover:shadow-md`,
    `scale-[0.90] group-hover:rotate-6 group-hover:translate-x-5 group-hover:-translate-y-2 z-10 -translate-y-2 group-hover:shadow-xl`,
];

function getLabel(item?: IotaObjectResponse) {
    if (!item) return;
    const display = item.data?.display?.data;
    return display?.name ?? display?.description ?? item.data?.objectId;
}

export function Kiosk({ object, orientation, ...nftImageProps }: KioskProps) {
    const address = useActiveAddress();
    const { data: kioskData, isPending } = useGetKioskContents(address);

    const kioskId = getKioskIdFromOwnerCap(object);
    const kiosk = kioskData?.kiosks.get(kioskId!);
    const itemsWithDisplay = kiosk?.items.filter((item) => hasDisplayData(item)) ?? [];

    const showCardStackAnimation = itemsWithDisplay.length > 1 && orientation !== 'horizontal';
    const imagesToDisplay = orientation !== 'horizontal' ? 3 : 1;
    const items = kiosk?.items.slice(0, imagesToDisplay) ?? [];

    // get the label for the first item to show on hover
    const displayName = getLabel(items[0]);

    if (isPending) return null;

    return (
        <div className="group relative h-36 w-36 transform-gpu overflow-visible rounded-xl hover:bg-transparent">
            <div className="absolute z-0">
                {itemsWithDisplay.length === 0 ? (
                    <NftImage animateHover src={null} name="Kiosk" {...nftImageProps} />
                ) : (
                    items.map((item, idx) => {
                        const display = item.data?.display?.data;
                        return (
                            <div
                                key={item.data?.objectId}
                                className={cl(
                                    'absolute rounded-xl border',
                                    TIMING,
                                    showCardStackAnimation ? cardStyles[idx] : '',
                                )}
                            >
                                <div
                                    className={`${
                                        idx === 0 && showCardStackAnimation ? CLIP_PATH : ''
                                    } ${TIMING}`}
                                >
                                    <NftImage
                                        {...nftImageProps}
                                        src={display?.image_url ?? null}
                                        animateHover={items.length <= 1}
                                        name="Kiosk"
                                    />
                                </div>
                            </div>
                        );
                    })
                )}
            </div>
            {orientation !== 'horizontal' && (
                <div
                    className={cl(
                        TIMING,
                        {
                            'group-hover:-translate-x-0.5 group-hover:scale-95':
                                showCardStackAnimation,
                        },
                        'absolute bottom-1.5 flex w-full items-center justify-end gap-3 overflow-hidden px-2',
                    )}
                >
                    {displayName ? (
                        <div className="flex items-center justify-center overflow-hidden rounded-md bg-white/90 px-2 py-1.5 opacity-0 group-hover:opacity-100">
                            <Text
                                variant="subtitleSmall"
                                weight="semibold"
                                mono
                                color="steel-darker"
                                truncate
                            >
                                {displayName}
                            </Text>
                        </div>
                    ) : null}

                    <div className="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-md bg-gray-100 text-white">
                        <Text variant="subtitle" weight="medium">
                            {kiosk?.items.length}
                        </Text>
                    </div>
                </div>
            )}
        </div>
    );
}
