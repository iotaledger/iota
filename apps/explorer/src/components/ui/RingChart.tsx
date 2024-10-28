// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import { Fragment } from 'react';

type Gradient = {
    deg?: number;
    values: { percent: number; color: string }[];
};

type RingChartData = {
    value: number;
    label: string;
    color?: string;
    gradient?: Gradient;
}[];

interface RingChartLegendProps {
    data: RingChartData;
}

function getColorFromGradient({ deg, values }: Gradient): string {
    const gradientResult = [];

    if (deg) {
        gradientResult.push(`${deg}deg`);
    }

    const valuesMap = values.map(({ percent, color }) => `${color} ${percent}%`);

    gradientResult.push(...valuesMap);

    return `linear-gradient(${gradientResult.join(',')})`;
}

export function RingChartLegend({ data }: RingChartLegendProps): JSX.Element {
    return (
        <>
            {data.map(({ color, gradient, label, value }) => {
                const colorDisplay = gradient ? getColorFromGradient(gradient) : color;

                return (
                    <div
                        className={clsx('flex items-center gap-xxs', value === 0 && 'hidden')}
                        key={label}
                    >
                        <div
                            style={{ background: colorDisplay }}
                            className="h-1.5 w-1.5 rounded-full"
                        />
                        <div className="text-label-md text-neutral-10">
                            {value} {label}
                        </div>
                    </div>
                );
            })}
        </>
    );
}

interface RingChartProperties {
    cx?: number;
    cy?: number;
    radius?: number;
    width?: number;
}

export interface RingChartProps extends RingChartProperties {
    data: RingChartData;
}
export function RingChart({
    data,
    cx = 25,
    cy = 25,
    radius = 20,
    width = 5,
}: RingChartProps): JSX.Element {
    const dashArray = 2 * Math.PI * radius;
    const startAngle = -90;
    const total = data.reduce((acc, { value }) => acc + value, 0);
    let filled = 0;

    const segments = data.map(({ value, label, color, gradient }, idx) => {
        const gradientId = `gradient-${idx}`;
        const ratio = (100 / total) * value;
        const angle = (filled * 360) / 100 + startAngle;
        const offset = dashArray - (dashArray * (ratio * 0.98)) / 100;
        filled += ratio;

        return (
            <Fragment key={label}>
                {gradient && (
                    <defs>
                        <linearGradient id={gradientId}>
                            {gradient.values.map(({ percent, color }, i) => (
                                <stop key={i} offset={percent} stopColor={color} />
                            ))}
                        </linearGradient>
                    </defs>
                )}
                <circle
                    cx={cx}
                    cy={cy}
                    r={radius}
                    fill="transparent"
                    stroke={gradient ? `url(#${gradientId})` : color}
                    strokeWidth={width}
                    strokeDasharray={dashArray}
                    strokeDashoffset={offset}
                    strokeLinecap="round"
                    transform={`rotate(${angle} ${cx} ${cy})`}
                />
            </Fragment>
        );
    });

    return (
        <div className="relative">
            <svg viewBox="0 0 50 50" strokeLinecap="round">
                {segments}
            </svg>
            <div className="absolute inset-0 mx-auto flex items-center justify-center">
                <div className="flex flex-col items-center gap-1.5">
                    <span className="text-title-md text-neutral-10">{total}</span>
                </div>
            </div>
        </div>
    );
}
