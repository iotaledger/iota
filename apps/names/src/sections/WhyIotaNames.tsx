// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import establish from '@/animations/landing/establish-your-present_optimized.json';
import oneName from '@/animations/landing/one-name-everywhere_optimized.json';
import ownTransfer from '@/animations/landing/own-transfer_optimized.json';
import { ImageCard } from '@/components/ImageCard';

const WHY_IOTA_NAMES = [
    {
        title: 'Establish Your Presence',
        body: 'Secure a company, product, or service name before someone else does - protect your namespace early',
        lottie: establish,
    },
    {
        title: 'Own. Transfer. Build On.',
        body: 'Your IOTA Names is a true on-chain asset - own it, sell it or transfer it',
        lottie: ownTransfer,
    },
    {
        title: 'One Name, Everywhere',
        body: 'Send payments and interact across any wallet or dApp with a familiar @name, not 64-character hashes, just simple, seamless UX',
        lottie: oneName,
    },
];

export function WhyIotaNames() {
    return (
        <section>
            <div className="-mt-40 md:-mt-20 pb-14 md:pb-20 container items-center">
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-lg">
                    {WHY_IOTA_NAMES.map((data, index) => (
                        <ImageCard key={index} {...data} />
                    ))}
                </div>
            </div>
        </section>
    );
}
