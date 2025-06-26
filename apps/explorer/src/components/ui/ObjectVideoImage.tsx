// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NFTMediaRenderer } from '@iota/core';
import { cva, type VariantProps } from 'class-variance-authority';
import clsx from 'clsx';

import { ObjectModal } from '~/components/ui';

const imageStyles = cva(['flex-shrink-0'], {
    variants: {
        objectFit: {
            cover: 'object-cover',
            contain: 'object-contain',
            fill: 'object-fill',
            none: 'object-none',
        },
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
        rounded: {
            full: 'rounded-full',
            '2xl': 'rounded-2xl',
            lg: 'rounded-lg',
            xl: 'rounded-xl',
            md: 'rounded-md',
            sm: 'rounded-sm',
            none: 'rounded-none',
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
    rounded?: ImageStylesProps['rounded'];
    disableVideoControls?: boolean;
    disableAutoPlay?: boolean;
}

export function ObjectVideoImage({
    objectFit = 'cover',
    title,
    subtitle,
    src,
    variant,
    open,
    setOpen,
    disablePreview,
    rounded = 'md',
    disableVideoControls,
    disableAutoPlay = false,
}: ObjectVideoImageProps): JSX.Element {
    const close = () => {
        if (disablePreview) {
            return;
        }

        if (setOpen) {
            setOpen(false);
        }
    };
    const openPreview = () => {
        if (disablePreview) {
            return;
        }

        if (setOpen) {
            setOpen(true);
        }
    };

    return (
        <>
            <ObjectModal
                open={!!open}
                onClose={close}
                title={title}
                subtitle={subtitle}
                src={src || ''}
                alt={title}
            />
            <div
                className={clsx(
                    'bg-iota-neutral-96 dark:bg-iota-neutral-10',
                    imageStyles({ variant, disablePreview, rounded }),
                    rounded && 'overflow-hidden',
                )}
                onClick={openPreview}
            >
                <NFTMediaRenderer
                    src={src}
                    alt={title}
                    disableVideoControls={disableVideoControls}
                    disableAutoPlay={disableAutoPlay}
                    objectFit={imageStyles({ objectFit })}
                />
            </div>
        </>
    );
}
