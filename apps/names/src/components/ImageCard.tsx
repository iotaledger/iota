// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';

import { LottieAnimation } from './loaders/Lottie';

export enum ImageCardSize {
    Large = 'large',
    Small = 'small',
}
interface ImageCardprops {
    title: string;
    body?: React.ReactNode;
    image?: string;
    alt?: string;
    size?: ImageCardSize;
    lottie?: React.ComponentProps<typeof LottieAnimation>['animationData'];
}
export function ImageCard({
    title,
    body,
    image,
    alt,
    lottie,
    size = ImageCardSize.Small,
}: ImageCardprops): JSX.Element {
    const isLarge = size === ImageCardSize.Large;
    const textSize = isLarge ? 'text-headline-sm' : 'text-title-lg';
    const isCentered = isLarge ? 'text-center' : 'text-start';
    return (
        <div className="w-full flex flex-col items-center overflow-hidden rounded-3xl bg-names-neutral-6">
            {lottie ? (
                <LottieAnimation animationData={lottie} />
            ) : (
                <img
                    src={image}
                    alt={alt || image}
                    className="w-full h-auto object-cover aspect-video"
                />
            )}

            <div
                className={clsx(
                    'flex flex-col pt-lg px-xl pb-xl self-stretch gap-x-xs',
                    isCentered,
                )}
            >
                <h2 className={clsx('text-names-primary-100', textSize)}>{title}</h2>
                {body && <p className="text-body-lg text-names-neutral-70">{body}</p>}
            </div>
        </div>
    );
}
