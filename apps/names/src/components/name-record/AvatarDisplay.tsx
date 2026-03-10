// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isSubname } from '@iota/iota-names-sdk';
import cx from 'clsx';
import { useEffect, useState } from 'react';

import loadingAnimationData from '@/animations/lottie-loading.json';
import { useNameRecord, useRegistrationNfts } from '@/hooks';
import { useGetObject } from '@/hooks/useGetOwnedObject';

import { LottieAnimation } from '../loaders/Lottie';

const FALLBACK_URL = '/name-bg.svg';

interface NameAvatarDisplay {
    name: string;
    blur?: boolean;
    isOnlyDefaultAvatar?: boolean;
}

export function NameAvatarDisplay({ name, blur, isOnlyDefaultAvatar = false }: NameAvatarDisplay) {
    const { data: nameRecordData, isLoading: isNameRecordDataLoading } = useNameRecord(name);
    const { data: subnames, isLoading: isSubnamesLoading } = useRegistrationNfts('subname');
    const isNameSubname = isSubname(name);

    const avatarId =
        nameRecordData?.type === 'unavailable'
            ? nameRecordData?.nameRecord.avatar && !isOnlyDefaultAvatar
                ? nameRecordData.nameRecord.avatar
                : isNameSubname
                  ? subnames?.find((n) => n.name === name)?.id
                  : nameRecordData.nameRecord.nftId
            : null;

    const { data: avatarObject, isLoading: isAvatarLoading } = useGetObject(name, {
        id: avatarId ?? '',
        options: { showDisplay: true, showContent: true },
    });

    const isDataLoading = isNameRecordDataLoading || isSubnamesLoading || isAvatarLoading;

    return (
        <AvatarDisplay
            src={avatarObject?.display?.data?.image_url}
            isLoadingSrc={isDataLoading}
            alt={name}
            blur={blur}
        />
    );
}

interface AvatarDisplayProps {
    src?: string;
    isLoadingSrc?: boolean;
    alt?: string;
    blur?: boolean;
}

export function AvatarDisplay({ src, alt, isLoadingSrc, blur }: AvatarDisplayProps) {
    const [srcStatus, setSrcStatus] = useState<'pending' | 'success' | 'error'>('pending');
    const [avatarStatus, setAvatarStatus] = useState<'loading' | 'success' | 'error'>('loading');

    const avatarSrc = (() => {
        if (srcStatus === 'error') {
            // Avatar src failed
            return FALLBACK_URL;
        } else if (src) {
            // Avatar src exists
            return src;
        } else if (!isLoadingSrc) {
            // Avatar src doesnt exist and it stopped loading
            return FALLBACK_URL;
        } else {
            // Avatar src doesnt exist and still loading
            return null;
        }
    })();

    useEffect(() => {
        if (!avatarSrc) return;

        const img = new Image();

        function handleStartLoad() {
            setAvatarStatus('loading');
        }

        function handleLoad() {
            if (avatarSrc === src) {
                setSrcStatus('success');
            }
            setAvatarStatus('success');
        }

        function handleError() {
            if (avatarSrc === src) {
                setSrcStatus('error');
            }
            setAvatarStatus('error');
        }

        img.addEventListener('loadstart', handleStartLoad);
        img.addEventListener('load', handleLoad);
        img.addEventListener('error', handleError);

        img.src = avatarSrc;

        return () => {
            img.removeEventListener('startload', handleStartLoad);
            img.removeEventListener('load', handleLoad);
            img.removeEventListener('error', handleError);
        };
    }, [avatarSrc]);

    const isLoading = srcStatus === 'pending' || avatarStatus === 'loading';

    return (
        <div className="flex flex-col relative rounded-xl overflow-hidden w-full h-full select-none">
            {isLoadingSrc || isLoading ? (
                <div className="w-full aspect-square relative">
                    <div className="absolute inset-0 w-full h-full flex items-center justify-center">
                        <LottieAnimation
                            animationData={loadingAnimationData}
                            className="w-16 h-16"
                        />
                    </div>
                    <img className={cx('w-full', 'block')} src="/name-bg.svg" />
                </div>
            ) : avatarSrc ? (
                <div className="w-full aspect-square relative">
                    <img
                        className={cx(
                            'absolute inset-0 w-full h-full object-cover',
                            blur && 'blur-sm',
                        )}
                        src={avatarSrc}
                        alt={alt}
                    />
                    <div className="absolute inset-0 w-full h-full flex items-center justify-center font-roboto-flex text-md font-semibold">
                        {avatarSrc === FALLBACK_URL ? <span>Couldn't load avatar</span> : null}
                    </div>
                </div>
            ) : null}
        </div>
    );
}
