// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useState } from 'react';
import { useGetIotaNameAvatar } from '../../hooks/useGetIotaNameAvatar';

interface NameAvatarProps {
    address: string | null | undefined;
    fallback: ReactNode;
    className?: string;
}

export function NameAvatar({ address, fallback, className }: NameAvatarProps) {
    const { data: avatarUrl, isLoading } = useGetIotaNameAvatar(address);
    const [imgError, setImgError] = useState(false);

    const showImage = !isLoading && !!avatarUrl && !imgError;

    if (showImage) {
        return (
            <img
                src={avatarUrl}
                alt="name avatar"
                className={className ?? 'h-full w-full rounded-full object-cover'}
                onError={() => setImgError(true)}
            />
        );
    }

    return <>{fallback}</>;
}
