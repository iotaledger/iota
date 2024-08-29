// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ReactNode } from 'react';
export type CardBodyProps = {
    title: string;
    subtitle?: string | ReactNode;
    clickableAction?: React.ReactNode;
};

export function CardBody({ title, subtitle, clickableAction }: CardBodyProps) {
    const handleActionCardBodyClick = (event: React.MouseEvent) => {
        event?.stopPropagation();
    };
    return (
        <div className="flex w-full flex-col">
            <div className="flex flex-row gap-x-xs">
                <div className="font-inter text-title-md text-neutral-10 dark:text-neutral-92">
                    {title}
                </div>
                {clickableAction && (
                    <div onClick={handleActionCardBodyClick}>{clickableAction}</div>
                )}
            </div>
            {subtitle && (
                <div className="font-inter text-body-md text-neutral-40 dark:text-neutral-60">
                    {subtitle}
                </div>
            )}
        </div>
    );
}
