// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type CardTextProps = {
    title?: string;
    subtitle?: string;
};

export function CardText({ title, subtitle }: CardTextProps) {
    return (
        <div>
            <div className={'font-inter text-title-lg text-neutral-10'}>{title}</div>
            {subtitle && (
                <div className={'font-inter text-body-md text-neutral-40'}>{subtitle}</div>
            )}
        </div>
    );
}
