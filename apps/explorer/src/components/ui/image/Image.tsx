// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator } from '@iota/apps-ui-kit';
import { PlaceholderReplace, VisibilityOff } from '@iota/ui-icons';
import { cva, cx, type VariantProps } from 'class-variance-authority';
import clsx from 'clsx';
import { useAnimate } from 'framer-motion';
import { type ImgHTMLAttributes, useEffect, useState } from 'react';
import { useImage } from '~/hooks';
import { ImageVisibility } from '~/lib/enums';

const imageStyles = cva(null, {
    variants: {
        rounded: {
            full: 'rounded-full',
            '2xl': 'rounded-2xl',
            lg: 'rounded-lg',
            xl: 'rounded-xl',
            md: 'rounded-md',
            sm: 'rounded-sm',
            none: 'rounded-none',
        },
        fit: {
            cover: 'object-cover',
            contain: 'object-contain',
            fill: 'object-fill',
            none: 'object-none',
            scaleDown: 'object-scale-down',
        },
        size: {
            sm: 'h-16 w-16',
            md: 'h-24 w-24',
            lg: 'h-32 w-32',
            full: 'h-full w-full',
        },
        aspect: {
            square: 'aspect-square',
        },
    },
    defaultVariants: {
        size: 'full',
        rounded: 'none',
        fit: 'cover',
    },
});

type ImageStyleProps = VariantProps<typeof imageStyles>;

export interface ImageProps extends ImageStyleProps, ImgHTMLAttributes<HTMLImageElement> {
    onClick?: () => void;
    moderate?: boolean;
    src: string;
    visibility?: ImageVisibility;
    fadeIn?: boolean;
}

function BaseImage({
    status,
    size,
    rounded,
    alt,
    src,
    srcSet,
    fit,
    visibility,
    onClick,
    fadeIn,
    aspect,
    ...imgProps
}: ImageProps & { status: string }): JSX.Element {
    const [scope, animate] = useAnimate();
    const [isBlurred, setIsBlurred] = useState(false);
    useEffect(() => {
        if (visibility && visibility !== ImageVisibility.Pass) {
            setIsBlurred(true);
        }
    }, [visibility]);

    const animateFadeIn = fadeIn && status === 'loaded';

    useEffect(() => {
        if (animateFadeIn) {
            animate(scope.current, { opacity: 1 }, { duration: 0.3 });
        }
    }, [animate, animateFadeIn, scope]);

    return (
        <div
            ref={scope}
            className={cx(
                imageStyles({ size, rounded, aspect }),
                'relative flex items-center justify-center bg-neutral-96 text-neutral-40 dark:bg-neutral-10 dark:text-neutral-60',
                animateFadeIn && 'opacity-0',
            )}
        >
            {status === 'loading' ? (
                <LoadingIndicator />
            ) : status === 'loaded' ? (
                isBlurred && (
                    <div
                        className={clsx(
                            'absolute z-20 flex h-full w-full items-center justify-center rounded-md bg-neutral-10/30 text-center text-white backdrop-blur-md',
                            visibility === ImageVisibility.Hide &&
                                'pointer-events-none cursor-not-allowed',
                        )}
                        onClick={() => setIsBlurred(!isBlurred)}
                    >
                        <VisibilityOff />
                    </div>
                )
            ) : status === 'failed' ? (
                <PlaceholderReplace />
            ) : null}
            {status === 'loaded' && (
                <img
                    alt={alt}
                    src={src}
                    srcSet={srcSet}
                    className={imageStyles({
                        rounded,
                        fit,
                        size,
                    })}
                    onClick={onClick}
                    {...imgProps}
                />
            )}
        </div>
    );
}

export function Image({ src, moderate = true, ...props }: ImageProps): JSX.Element {
    const { status, url, moderation } = useImage({ src, moderate });
    return <BaseImage visibility={moderation?.visibility} status={status} src={url} {...props} />;
}
