// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonUnstyled } from '@iota/apps-ui-kit';
import { Pause, Play } from '@iota/apps-ui-icons';
import { motion } from 'framer-motion';
import { useEffect } from 'react';

const getAnimationVariants = (duration: number) => ({
    initial: {
        pathLength: 0,
    },
    animate: {
        pathLength: 1,
        transition: {
            duration: duration,
        },
    },
});

export interface PlayPauseProps {
    paused?: boolean;
    onChange(): void;
    animate?: {
        duration: number;
        start: boolean;
        setStart: (bool: boolean) => void;
    };
}

export function PlayPause({ paused, onChange, animate }: PlayPauseProps): JSX.Element {
    const Icon = paused ? Play : Pause;

    const isAnimating = animate?.start && !paused;

    useEffect(() => {
        let timer: NodeJS.Timeout;

        if (isAnimating) {
            timer = setTimeout(() => {
                animate.setStart(false);
            }, animate.duration * 1000);
        }

        return () => clearTimeout(timer);
    }, [animate, isAnimating]);

    return (
        <ButtonUnstyled
            aria-label={paused ? 'Paused' : 'Playing'}
            onClick={onChange}
            className="relative cursor-pointer border-none bg-transparent p-xxs text-neutral-40 dark:text-neutral-60"
        >
            {isAnimating && (
                <motion.svg
                    className="absolute left-1/2 top-1/2 h-full w-full -translate-x-1/2 -translate-y-1/2 -rotate-90 text-primary-60"
                    viewBox="0 0 16 16"
                >
                    <motion.circle
                        fill="none"
                        cx="8"
                        cy="8"
                        r="7"
                        strokeLinecap="round"
                        strokeWidth={1.5}
                        stroke="currentColor"
                        variants={getAnimationVariants(animate.duration)}
                        initial="initial"
                        animate="animate"
                    />
                </motion.svg>
            )}
            <Icon />
        </ButtonUnstyled>
    );
}
