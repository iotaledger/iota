// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';

import { SvgCircleGradient } from './svgs/CircleGradient';

interface CircleGradientProps {
    position?: 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
}

export function CircleGradient({ position = 'top-left' }: CircleGradientProps) {
    const base = 'absolute pointer-events-none select-none z-[-1]';

    const positionClass = {
        'top-left': 'top-0 left-0 -translate-x-1/2 -translate-y-1/4',
        'top-right': 'top-0 right-0 translate-x-1/2 -translate-y-1/4',
        'bottom-left': 'bottom-0 left-0 -translate-x-1/2 translate-y-1/4',
        'bottom-right': 'bottom-0 right-0 translate-x-1/2 translate-y-1/4',
    }[position];

    return <SvgCircleGradient aria-hidden="true" className={clsx(base, positionClass)} />;
}
