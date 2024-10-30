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

interface FallBackAvatarProps {
    text: string;
    rounded?: boolean;
    size?: ImageIconSize;
}
function FallBackAvatar({ text, rounded, size = ImageIconSize.Large }: FallBackAvatarProps) {
    const textSize = (() => {
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
    })();

    return (
        <div
            className={cn(
                'flex h-full w-full items-center justify-center bg-neutral-96 bg-gradient-to-r capitalize dark:bg-neutral-20',
                { 'rounded-full': rounded },
                textSize,
            )}
        >
            {text.slice(0, 2)}
        </div>
    );
}
export interface ImageIconProps {
    src: string | null | undefined;
    label: string;
    fallbackText: string;
    alt?: string;
    rounded?: boolean;
    size?: ImageIconSize;
}

export function ImageIcon({
    src,
    label,
    alt = label,
    fallbackText,
    rounded,
    size,
}: ImageIconProps) {
    const [error, setError] = useState(false);
    return (
        <div role="img" aria-label={label} className={size}>
            {error || !src ? (
                <FallBackAvatar rounded={rounded} text={fallbackText} size={size} />
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
