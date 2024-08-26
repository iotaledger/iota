// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { motion, type Variants } from 'framer-motion';

const ANIMATION_START = 0.25;

const getProgressBarVariant = (progress: number): Variants => ({
    initial: {
        width: 0,
    },
    animate: {
        transition: {
            delay: ANIMATION_START,
            duration: 0.5,
            delayChildren: ANIMATION_START * 8,
        },
        width: `${progress}%`,
    },
});

export interface ProgressBarProps {
    progress: number;
}

export function ProgressBar({ progress }: ProgressBarProps): JSX.Element {
    return (
        <div className="px-md--rs py-sm--rs">
            <div className="relative h-1 w-full rounded-full bg-primary-90">
                <motion.div
                    variants={getProgressBarVariant(progress)}
                    className="rounded-full bg-primary-30 py-1"
                    initial="initial"
                    animate="animate"
                />
            </div>
        </div>
    );
}
