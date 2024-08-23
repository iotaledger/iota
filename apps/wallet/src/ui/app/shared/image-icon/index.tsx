// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';

export interface ImageIconProps {
    src: string | null | undefined;
    label: string;
    fallback: string;
    alt?: string;
}

function FallBackAvatar({ str }: { str: string }) {
    return (
        <div className="flex h-10 w-10 items-center justify-center  rounded-full bg-primary-40 bg-gradient-to-r text-label-md text-primary-100">
            {str?.slice(0, 2)}
        </div>
    );
}

export function ImageIcon({ src, label, alt = label, fallback }: ImageIconProps) {
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
