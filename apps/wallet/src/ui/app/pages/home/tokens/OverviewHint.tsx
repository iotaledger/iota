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
            className="state-layer relative flex w-full cursor-pointer items-center gap-3 rounded-xl  border border-transparent bg-iota-neutral-96 p-xs px-sm py-xs dark:bg-iota-neutral-10"
            onClick={onClick}
        >
            <IconComponent className="h-5 w-5 text-iota-primary-20 dark:text-iota-primary-90" />
            <span className="text-label-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                {title}
            </span>
        </div>
    );
}
