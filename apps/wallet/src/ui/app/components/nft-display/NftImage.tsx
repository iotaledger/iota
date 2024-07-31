// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Image32, LockLocked16, MediaPlay16 } from '@iota/icons';
import { cva } from 'class-variance-authority';
import type { VariantProps } from 'class-variance-authority';
import cl from 'clsx';
import { useState } from 'react';

const nftImageStyles = cva('overflow-hidden bg-gray-40 relative', {
    variants: {
        animateHover: {
            true: [
                'ease-ease-out-cubic duration-400',
                'group-hover:shadow-blurXl group-hover:shadow-steel/50',
            ],
        },
        borderRadius: {
            md: 'rounded-md',
            xl: 'rounded-xl',
            sm: 'rounded',
        },
        size: {
            xs: 'w-10 h-10',
            sm: 'w-12 h-12',
            md: 'w-24 h-24',
            lg: 'w-36 h-36',
            xl: 'w-50 h-50',
        },
    },
    compoundVariants: [
        {
            animateHover: true,
            borderRadius: 'xl',
            class: 'group-hover:rounded-md',
        },
    ],
    defaultVariants: {
        borderRadius: 'md',
    },
});

export interface NftImageProps extends VariantProps<typeof nftImageStyles> {
    src: string | null;
    video?: string | null;
    name: string | null;
    title?: string;
    showLabel?: boolean;
    playable?: boolean;
    className?: string;
    isLocked?: boolean;
}

export function NftImage({
    src,
    name,
    title,
    showLabel,
    animateHover,
    borderRadius,
    size,
    video,
    playable,
    className,
    isLocked,
}: NftImageProps) {
    const [error, setError] = useState(false);
    const imgCls = cl(
        'w-full h-full object-cover',
        animateHover && 'group-hover:scale-110 duration-500 ease-ease-out-cubic',
        className,
    );
    const imgSrc = src ? src.replace(/^ipfs:\/\//, 'https://ipfs.io/ipfs/') : '';

    return (
        <div
            className={nftImageStyles({
                animateHover,
                borderRadius,
                size,
            })}
        >
            {video ? (
                playable ? (
                    <video
                        autoPlay
                        muted
                        controls
                        className="h-full w-full overflow-hidden rounded-md object-cover"
                        src={video}
                    />
                ) : (
                    <div className="pointer-events-none absolute bottom-2 right-2 z-10 flex items-center justify-center rounded-full text-black opacity-80">
                        <MediaPlay16 className="h-8 w-8" />
                    </div>
                )
            ) : error || !imgSrc ? (
                <div
                    className={cl(
                        imgCls,
                        'flex flex-col flex-nowrap items-center justify-center',
                        'text-steel-dark select-none gap-2 bg-placeholderGradient01 uppercase',
                    )}
                    title={title}
                >
                    <Image32 className="text-steel h-6 w-6 text-3xl" />
                    {showLabel ? (
                        <span className="text-captionSmall font-medium">No media</span>
                    ) : null}
                </div>
            ) : (
                <img
                    className={imgCls}
                    src={imgSrc}
                    alt={name || 'NFT'}
                    title={title}
                    onError={() => setError(true)}
                />
            )}
            {isLocked ? (
                <div className="absolute bottom-1.5 right-1.5 flex h-6 w-6 items-center justify-center rounded-md bg-gray-100 text-white">
                    <LockLocked16 className="h-3.5 w-3.5" />
                </div>
            ) : null}
        </div>
    );
}
