// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import Image from 'next/image';
import cn from 'clsx';

export enum ImageIconSize {
    Small = 'w-5 h-5',
    Medium = 'w-8 h-8',
    Large = 'w-10 h-10',
    Full = 'w-full h-full',
}

export interface ImageIconProps {
    src: string | null | undefined;
    label: string;
    fallback: string;
    alt?: string;
    rounded?: boolean;
    size?: ImageIconSize;
}

function FallBackAvatar({
    str,
    rounded,
    size = ImageIconSize.Large,
}: {
    str: string;
    rounded?: boolean;
    size?: ImageIconSize;
}) {
    function generateTextSize(size: ImageIconSize) {
        switch (size) {
            case ImageIconSize.Small:
                return 'text-label-sm';
            case ImageIconSize.Medium:
                return 'text-label-md';
            case ImageIconSize.Large:
                return 'text-title-md';
            case ImageIconSize.Full:
                return 'text-title-lg';
        }
    }
    return (
        <div
            className={cn(
                'flex h-full w-full items-center justify-center bg-neutral-96 bg-gradient-to-r capitalize dark:bg-neutral-20',
                { 'rounded-full': rounded },
                generateTextSize(size),
            )}
        >
            {str?.slice(0, 2)}
        </div>
    );
}

export function ImageIcon({ src, label, alt = label, fallback, rounded, size }: ImageIconProps) {
    const [error, setError] = useState(false);
    return (
        <div role="img" aria-label={label} className={size}>
            {error || !src ? (
                <FallBackAvatar rounded={rounded} str={fallback} size={size} />
            ) : (
                <Image
                    src={src}
                    alt={alt}
                    className="flex h-full w-full items-center justify-center rounded-full object-cover"
                    onError={() => setError(true)}
                    layout="fill"
                    objectFit="cover"
                />
            )}
        </div>
    );
}
