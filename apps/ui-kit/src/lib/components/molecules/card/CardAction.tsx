// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType } from '@/components/atoms/button';
import { CardActionVariant } from './card.enums';

export type CardActionProps = {
    title?: string;
    subtitle?: string;
    variant?: CardActionVariant;
    onClick?: () => void;
};

export function CardAction({ variant, onClick, subtitle, title }: CardActionProps) {
    if (variant === CardActionVariant.Link) {
        return (
            <div onClick={onClick}>
                <svg
                    className={'text-neutral-10 dark:text-neutral-92'}
                    xmlns="http://www.w3.org/2000/svg"
                    width="24"
                    height="24"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M10.7071 6.29289L15.7071 11.2929C16.0976 11.6834 16.0976 12.3166 15.7071 12.7071L10.7071 17.7071C10.3166 18.0976 9.68342 18.0976 9.29289 17.7071C8.90237 17.3166 8.90237 16.6834 9.29289 16.2929L13.5858 12L9.29289 7.70711C8.90237 7.31658 8.90237 6.68342 9.29289 6.29289C9.68342 5.90237 10.3166 5.90237 10.7071 6.29289Z"
                        fill="currentColor"
                    />
                </svg>
            </div>
        );
    }

    if (variant === CardActionVariant.SupportingText) {
        return (
            <div className={'text-right'}>
                {title && (
                    <div
                        className={'font-inter text-label-md text-neutral-10 dark:text-neutral-92'}
                    >
                        {title}
                    </div>
                )}
                {subtitle && (
                    <div
                        className={'font-inter text-label-sm text-neutral-40 dark:text-neutral-60'}
                    >
                        {subtitle}
                    </div>
                )}
            </div>
        );
    }
    if (variant === CardActionVariant.Button) {
        return (
            <div>
                <Button
                    type={ButtonType.Outlined}
                    size={ButtonSize.Small}
                    text={title}
                    onClick={onClick}
                />
            </div>
        );
    }

    return null;
}
