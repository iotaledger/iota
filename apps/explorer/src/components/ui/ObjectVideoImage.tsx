// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator, VisualAssetType } from '@iota/apps-ui-kit';
import { NFTVideoAsset, useResolveNFTMedia } from '@iota/core';
import { cva, type VariantProps } from 'class-variance-authority';
import clsx from 'clsx';
import { useRef } from 'react';

import { Image, ObjectModal, type ImageProps } from '~/components/ui';

const imageStyles = cva(['flex-shrink-0'], {
    variants: {
        variant: {
            xxs: 'h-8 w-8',
            xs: 'h-12 w-12',
            small: 'h-16 w-16',
            medium: 'md:h-31.5 md:w-31.5 h-16 w-16',
            large: 'h-50 w-50',
            fill: 'h-full w-full',
        },
        disablePreview: {
            true: '',
            false: 'cursor-pointer',
        },
    },
    defaultVariants: {
        disablePreview: false,
    },
});

type ImageStylesProps = VariantProps<typeof imageStyles>;

interface ObjectVideoImageProps extends ImageStylesProps {
    title: string;
    subtitle: string;
    src: string;
    open?: boolean;
    setOpen?: (open: boolean) => void;
    rounded?: ImageProps['rounded'];
    disablePreview?: boolean;
    fadeIn?: boolean;
    imgFit?: ImageProps['fit'];
    aspect?: ImageProps['aspect'];
    disableVideoControls?: boolean;
}

export function ObjectVideoImage({
    title,
    subtitle,
    src,
    variant,
    open,
    setOpen,
    disablePreview,
    fadeIn,
    imgFit,
    aspect,
    rounded = 'md',
    disableVideoControls,
}: ObjectVideoImageProps): JSX.Element {
    const { data: resolvedNFTInfo, isLoading } = useResolveNFTMedia(src);
    const videoRef = useRef<HTMLVideoElement | null>(null);

    const close = () => {
        if (disablePreview || isLoading) {
            return;
        }

        if (setOpen) {
            setOpen(false);
        }

        videoRef.current?.play();
    };
    const openPreview = () => {
        if (disablePreview || isLoading) {
            return;
        }

        if (setOpen) {
            setOpen(true);
        }

        videoRef.current?.pause();
    };

    const isAssetVideo = resolvedNFTInfo?.assetType === VisualAssetType.Video;

    return (
        <>
            <ObjectModal
                open={!!open}
                onClose={close}
                title={title}
                subtitle={subtitle}
                src={resolvedNFTInfo?.src || ''}
                alt={title}
            />
            <div
                className={clsx(
                    imageStyles({ variant, disablePreview }),
                    isAssetVideo && 'group/video',
                )}
                onClick={openPreview}
            >
                {isAssetVideo ? (
                    isLoading ? (
                        <LoadingIndicator />
                    ) : (
                        <div className="pointer-events-none flex h-full w-full items-center justify-center">
                            <NFTVideoAsset
                                src={resolvedNFTInfo.src}
                                isAutoPlayEnabled={resolvedNFTInfo.isAutoPlayEnabled}
                                ref={videoRef}
                                disableControls={disableVideoControls}
                            />
                        </div>
                    )
                ) : (
                    <Image
                        aspect={aspect}
                        rounded={rounded}
                        alt={title}
                        src={src}
                        fadeIn={fadeIn}
                        fit={imgFit}
                    />
                )}
            </div>
        </>
    );
}
