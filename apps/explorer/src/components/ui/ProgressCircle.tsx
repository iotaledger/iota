// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface ProgressCircleProps {
    progress: number;
}

export function ProgressCircle({ progress }: ProgressCircleProps): JSX.Element {
    const radius = 5;
    const circumference = 2 * Math.PI * radius;
    const strokeDashoffset = circumference * (1 - (progress === 100 ? 1.5 : progress / 100));

    return (
        <svg className="rotate-90" viewBox="0 0 16 16">
            <circle
                fill="none"
                cx="8"
                cy="8"
                r={radius}
                strokeLinecap={progress === 100 ? 'butt' : 'round'}
                strokeWidth={1.5}
                stroke="currentColor"
                strokeDasharray={circumference}
                strokeDashoffset={strokeDashoffset}
                style={{
                    transition: 'stroke-dashoffset 1s ease-in-out',
                }}
            />
        </svg>
    );
}
