// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import simplifiedTransactions from '@/animations/landing/simplified-safer-trans_optimized.json';
import unlimitedSubnames from '@/animations/landing/unlimited-subnames_optimized.json';
import { CircleGradient } from '@/components/CircleGradient';
import { ImageCard, ImageCardSize } from '@/components/ImageCard';

const WHY_IOTA_NAMES = [
    {
        title: 'Simplified, Safer Transactions',
        body: 'Replace long, error-prone hashes with names people can recognize and remember',
        lottie: simplifiedTransactions,
    },
    {
        title: 'Unlimited Subnames, Infinite Utility',
        body: 'Unlimited subnames let you set up custom tags like wallet@yourname or dev@yourname - ideal for services, roles, or communities',
        lottie: unlimitedSubnames,
    },
];

export function NameUtility() {
    return (
        <section className="relative overflow-hidden">
            <div className="container py-14 md:py-20 items-center">
                <div className="relative grid grid-cols-1 sm:grid-cols-2 gap-[33px]">
                    <CircleGradient position="top-right" />
                    <CircleGradient position="bottom-left" />
                    {WHY_IOTA_NAMES.map((data, index) => (
                        <ImageCard key={index} {...data} size={ImageCardSize.Large} />
                    ))}
                </div>
            </div>
        </section>
    );
}
