// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowRight } from '@iota/ui-icons';
import { Button, ButtonSize, ButtonType } from '@/components/atoms/button';
import { CardActionType } from './card.enums';

export type CardActionProps = {
    title?: string;
    subtitle?: string;
    type: CardActionType;
    onClick?: () => void;
    icon?: React.ReactNode;
};

export function CardAction({ type, onClick, subtitle, title, icon }: CardActionProps) {
    function handleActionClick(event: React.MouseEvent) {
        event?.stopPropagation();
        onClick?.();
    }

    if (type === CardActionType.Link) {
        return (
            <div
                onClick={handleActionClick}
                className="shrink-0 text-neutral-10 dark:text-neutral-92 [&_svg]:h-5 [&_svg]:w-5"
            >
                {icon ? icon : <ArrowRight />}
            </div>
        );
    }

    if (type === CardActionType.SupportingText) {
        return (
            <div className="shrink-0 text-right">
                {title && (
                    <div className="font-inter text-label-md text-neutral-10 dark:text-neutral-92">
                        {title}
                    </div>
                )}
                {subtitle && (
                    <div className="font-inter text-label-sm text-neutral-60 dark:text-neutral-40">
                        {subtitle}
                    </div>
                )}
            </div>
        );
    }
    if (type === CardActionType.Button) {
        return (
            <div className="shrink-0">
                <Button
                    type={ButtonType.Outlined}
                    size={ButtonSize.Small}
                    text={title}
                    onClick={handleActionClick}
                />
            </div>
        );
    }

    return null;
}
