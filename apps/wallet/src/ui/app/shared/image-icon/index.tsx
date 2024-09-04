// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import cn from 'clsx';

export interface ImageIconProps {
    src: string | null | undefined;
    label: string;
    fallback: string;
    alt?: string;
    rounded?: boolean;
}

function FallBackAvatar({ str, rounded }: { str: string; rounded?: boolean }) {
    return (
        <div
            className={cn(
                'flex h-10 w-10 items-center justify-center bg-primary-40 bg-gradient-to-r text-label-md text-primary-100',
                { 'rounded-full': rounded },
            )}
        >
            {str?.slice(0, 2)}
        </div>
    );
}

export function ImageIcon({ src, label, alt = label, fallback, rounded = true }: ImageIconProps) {
    const [error, setError] = useState(false);
    return (
        <div role="img" aria-label={label}>
            {error || !src ? (
                <FallBackAvatar str={fallback} />
            ) : (
                <img
                    src={src}
                    alt={alt}
                    className="flex h-full w-full items-center justify-center rounded-full object-cover"
                    onError={() => setError(true)}
                />
            )}
        </div>
    );
}
