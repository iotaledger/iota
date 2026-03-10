// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Lottie from 'lottie-react';

interface LottieAnimationProps {
    className?: string;
    animationData: unknown;
}

export function LottieAnimation({ className, animationData }: LottieAnimationProps) {
    return (
        <div className={className}>
            <Lottie
                animationData={animationData}
                loop={true}
                autoplay={true}
                style={{ width: '100%', height: '100%' }}
            />
        </div>
    );
}
