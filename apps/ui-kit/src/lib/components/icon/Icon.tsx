// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Suspense, useMemo } from 'react';
import { IconEnum, IconList } from './icons';

interface IconProps {
    /**
     * Icon to display.
     */
    icon: IconEnum;
    /**
     * Width of the icon.
     */
    width?: number;
    /**
     * Height of the icon.
     */
    height?: number;
    /**
     * Classname for the SVG element.
     */
    className?: string;
}

export function Icon({ icon, width = 24, height = 24, className }: IconProps) {
    const SvgIcon = useMemo(() => IconList[icon], [icon]);

    if (!SvgIcon) return null;

    return (
        <Suspense fallback={null}>
            <SvgIcon width={width} height={height} className={className} />
        </Suspense>
    );
}
