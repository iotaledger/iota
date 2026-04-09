// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface SummaryListItemProps {
    icon: React.ReactNode;
    text: React.ReactNode;
}

export function SummaryListItem({ icon, text }: SummaryListItemProps) {
    return (
        <div className="flex flex-row items-center gap-x-sm">
            {icon}
            <span className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                {text}
            </span>
        </div>
    );
}
