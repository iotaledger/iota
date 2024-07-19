// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type CardTextProps = {
    title: string;
    subtitle?: string;
};

export function CardText({ title, subtitle }: CardTextProps) {
    return (
        <div>
            <div className={'font-inter text-title-lg text-neutral-10 dark:text-neutral-92'}>
                {title}
            </div>
            {subtitle && (
                <div className={'font-inter text-body-md text-neutral-40 dark:text-neutral-60'}>
                    {subtitle}
                </div>
            )}
        </div>
    );
}
