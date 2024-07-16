// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType } from '@/components/atoms/button';

export enum CardActionVariant {
    None = 'none',
    Link = 'link',
    SupportingText = 'supporting-text',
    Button = 'button',
}

export type CardActionProps = {
    actionTitle?: string;
    actionSubtitle?: string;
    actionVariant?: CardActionVariant;
    actionOnClick?: () => void;
};

export function CardAction({
    actionVariant,
    actionOnClick,
    actionSubtitle,
    actionTitle,
}: CardActionProps) {
    if (actionVariant === CardActionVariant.Link) {
        return (
            <div onClick={actionOnClick}>
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="24"
                    height="24"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M10.7071 6.29289L15.7071 11.2929C16.0976 11.6834 16.0976 12.3166 15.7071 12.7071L10.7071 17.7071C10.3166 18.0976 9.68342 18.0976 9.29289 17.7071C8.90237 17.3166 8.90237 16.6834 9.29289 16.2929L13.5858 12L9.29289 7.70711C8.90237 7.31658 8.90237 6.68342 9.29289 6.29289C9.68342 5.90237 10.3166 5.90237 10.7071 6.29289Z"
                        fill="#171D26"
                    />
                </svg>
            </div>
        );
    }

    if (actionVariant === CardActionVariant.SupportingText) {
        return (
            <div className={'text-right'}>
                {actionTitle && (
                    <div className={'font-inter text-label-md text-neutral-10'}>{actionTitle}</div>
                )}
                {actionSubtitle && (
                    <div className={'font-inter text-label-sm text-neutral-40'}>
                        {actionSubtitle}
                    </div>
                )}
            </div>
        );
    }
    if (actionVariant === CardActionVariant.Button) {
        return (
            <div>
                <Button
                    type={ButtonType.Outlined}
                    size={ButtonSize.Small}
                    text={actionTitle}
                    onClick={actionOnClick}
                />
            </div>
        );
    }

    return null;
}
