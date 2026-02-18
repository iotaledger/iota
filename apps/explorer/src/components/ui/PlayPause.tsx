// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonUnstyled } from '@iota/apps-ui-kit';
import { Pause, Play } from '@iota/apps-ui-icons';
import { useEffect, useState } from 'react';

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
    const [animationProgress, setAnimationProgress] = useState(0);

    const isAnimating = animate?.start && !paused;

    useEffect(() => {
        let timer: NodeJS.Timeout;
        let animationFrame: number;

        if (isAnimating && animate) {
            const startTime = Date.now();
            const duration = animate.duration * 1000;

            const updateProgress = () => {
                const elapsed = Date.now() - startTime;
                const progress = Math.min(elapsed / duration, 1);
                setAnimationProgress(progress);

                if (progress < 1) {
                    animationFrame = requestAnimationFrame(updateProgress);
                }
            };

            updateProgress();

            timer = setTimeout(() => {
                animate.setStart(false);
                setAnimationProgress(0);
            }, duration);
        } else {
            setAnimationProgress(0);
        }

        return () => {
            clearTimeout(timer);
            if (animationFrame) {
                cancelAnimationFrame(animationFrame);
            }
        };
    }, [animate, isAnimating]);

    const radius = 7;
    const circumference = 2 * Math.PI * radius;
    const strokeDashoffset = circumference * (1 - animationProgress);

    return (
        <ButtonUnstyled
            aria-label={paused ? 'Paused' : 'Playing'}
            onClick={onChange}
            className="relative cursor-pointer border-none bg-transparent p-xxs text-iota-neutral-40 dark:text-iota-neutral-60"
        >
            {isAnimating && (
                <svg
                    className="absolute left-1/2 top-1/2 h-full w-full -translate-x-1/2 -translate-y-1/2 -rotate-90 text-iota-primary-60"
                    viewBox="0 0 16 16"
                >
                    <circle
                        fill="none"
                        cx="8"
                        cy="8"
                        r={radius}
                        strokeLinecap="round"
                        strokeWidth={1.5}
                        stroke="currentColor"
                        strokeDasharray={circumference}
                        strokeDashoffset={strokeDashoffset}
                    />
                </svg>
            )}
            <Icon />
        </ButtonUnstyled>
    );
}
