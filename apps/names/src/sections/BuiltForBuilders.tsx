// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CheckmarkFilled } from '@iota/apps-ui-icons';

import builtForBuilders from '@/animations/landing/build_for_builders_optimized.json';
import { LottieAnimation } from '@/components/loaders/Lottie';

interface FeatureItemProps {
    text: string;
}

export function FeatureItem({ text }: FeatureItemProps) {
    return (
        <div className="flex items-start gap-xs">
            <CheckmarkFilled className="text-names-tertiary-70 h-lg w-lg" />
            <p className="text-title-lg text-names-primary-100">{text}</p>
        </div>
    );
}

const FEATURES = [
    'Plugin tags for dApp discovery',
    'Anchor points for tokenized assets',
    'Public-facing handles for services',
];

export function BuiltForBuilders() {
    return (
        <section>
            <div className="container pt-14 pb-28 md:pt-20 md:pb-40 items-center">
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-lg bg-names-neutral-6 rounded-3xl  items-center">
                    <div className="flex flex-col gap-lg py-[40px] px-2xl">
                        <div className="flex flex-col gap-xs">
                            <h2 className="text-headline-lg text-names-primary-100">
                                Built for Builders
                            </h2>
                            <p className="text-body-lg text-names-neutral-70">
                                Think of IOTA Names as a flexible naming layer — open to evolve with
                                the ecosystem. They can serve as:
                            </p>
                        </div>
                        {FEATURES.map((text, index) => (
                            <FeatureItem key={index} text={text} />
                        ))}
                    </div>
                    <div className="flex justify-center">
                        <LottieAnimation animationData={builtForBuilders} />
                    </div>
                </div>
            </div>
        </section>
    );
}
