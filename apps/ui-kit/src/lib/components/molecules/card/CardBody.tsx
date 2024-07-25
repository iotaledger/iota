// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type CardBodyProps = {
    title: string;
    subtitle?: string;
};

export function CardBody({ title, subtitle }: CardBodyProps) {
    return (
        <div className="flex w-full flex-col">
            <div className="font-inter text-title-md text-neutral-10 dark:text-neutral-92">
                {title}
            </div>
            {subtitle && (
                <div className="font-inter text-body-md text-neutral-40 dark:text-neutral-60">
                    {subtitle}
                </div>
            )}
        </div>
    );
}
