// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ElementType } from 'react';

interface OverviewHintProps {
    onClick: () => void;
    icon: ElementType;
    title: string;
}

export function OverviewHint({ onClick, icon, title }: OverviewHintProps) {
    const IconComponent = icon;
    return (
        <div
            className="state-layer relative flex w-full cursor-pointer items-center gap-3 rounded-xl border border-transparent bg-warning-90 p-xs px-sm py-xs dark:bg-warning-20"
            onClick={onClick}
        >
            <IconComponent className="h-5 w-5 text-warning-10 dark:text-warning-90" />
            <span className="text-label-sm text-neutral-10 dark:text-neutral-92">{title}</span>
        </div>
    );
}
