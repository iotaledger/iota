// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getKioskIdFromOwnerCap, hasDisplayData, useGetKioskContents } from '@iota/core';
import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { useActiveAddress } from '_hooks';
import { ButtonUnstyled, CardImage, ImageType, truncate } from '@iota/apps-ui-kit';
import { PlaceholderReplace } from '@iota/ui-icons';

interface KioskProps {
    object: IotaObjectResponse;
}

export function Kiosk({ object }: KioskProps) {
    const address = useActiveAddress();
    const { data: kioskData, isPending } = useGetKioskContents(address);

    const kioskId = getKioskIdFromOwnerCap(object);
    const kiosk = kioskData?.kiosks.get(kioskId!);
    const itemsWithDisplay = kiosk?.items.filter((item) => hasDisplayData(item)) ?? [];

    const items = kiosk?.items ?? [];
    const displayBackgroundImage =
        itemsWithDisplay.length === 0
            ? null
            : itemsWithDisplay[0].data?.display?.data?.image_url || null;

    if (isPending) return null;

    return (
        <div className="group relative aspect-square w-full cursor-pointer overflow-hidden rounded-xl">
            <div
                className={
                    'group relative aspect-square w-full cursor-pointer overflow-hidden rounded-xl'
                }
            >
                <div className="absolute left-0 top-0 h-full w-full bg-cover bg-center bg-no-repeat group-hover:bg-shader-neutral-light-48 group-hover:transition group-hover:duration-300 group-hover:ease-in-out group-hover:dark:bg-shader-primary-dark-48" />
                <div className="relative flex aspect-square h-full w-full items-center justify-center overflow-hidden rounded-xl">
                    {displayBackgroundImage ? (
                        <img
                            src={displayBackgroundImage}
                            alt={kioskId}
                            className="h-full w-full object-cover"
                        />
                    ) : (
                        <CardImage type={ImageType.BgTransparent}>
                            <PlaceholderReplace className="text-neutral-40" />
                        </CardImage>
                    )}
                </div>
                <ButtonUnstyled className="absolute right-2 top-2 h-9 w-9 cursor-pointer rounded-full p-xs opacity-0 transition-opacity duration-300 group-hover:bg-shader-neutral-light-72 group-hover:opacity-100 [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-primary-100">
                    <span className="text-neutral-90">{items.length}</span>
                </ButtonUnstyled>
                <div className="absolute bottom-0 flex items-center justify-center p-xs opacity-0 transition-opacity duration-300 group-hover:opacity-100">
                    <span className="text-title-md text-neutral-100">{truncate(kioskId)}</span>
                </div>
            </div>
        </div>
    );
}
