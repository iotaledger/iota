// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ElementType } from 'react';

interface OverviewHintProps {
    onClick: () => void;
    icon: ElementType;
    title: string;
    subtitle?: string;
}

export function OverviewHint({ onClick, icon, title, subtitle }: OverviewHintProps) {
    const IconComponent = icon;
    return (
        <div
            className="state-layer dark:bg-iota-warning-20 relative flex w-full cursor-pointer items-center gap-3 rounded-xl border border-transparent bg-iota-warning-90 p-xs px-sm py-xs"
            onClick={onClick}
        >
            <IconComponent className="dark:text-iota-warning-90 h-5 w-5 text-iota-warning-10" />
            <div className="flex flex-col text-label-sm">
                <span className="dark:text-iota-neutral-92 text-iota-neutral-10">{title}</span>
                {subtitle && (
                    <span className="dark:text-iota-neutral-60 text-iota-neutral-40">
                        {subtitle}
                    </span>
                )}
            </div>
        </div>
    );
}
