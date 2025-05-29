// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cva, cx, type VariantProps } from 'class-variance-authority';
import { useAnimate } from 'framer-motion';
import { type ImgHTMLAttributes, useEffect } from 'react';

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
    src: string;
    fadeIn?: boolean;
}

export function Image({
    size,
    rounded,
    alt,
    src,
    srcSet,
    fit,
    onClick,
    fadeIn,
    aspect,
    ...imgProps
}: ImageProps): JSX.Element {
    const [scope, animate] = useAnimate();

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
        </div>
    );
}
