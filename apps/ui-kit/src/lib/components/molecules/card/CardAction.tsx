// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowRight } from '@iota/apps-ui-icons';
import { CardActionType } from './card.enums';
import { Button, ButtonSize, ButtonType } from '@/components/atoms/button';

export type CardActionProps = {
    title?: string;
    subtitle?: string;
    type: CardActionType;
    onClick?: () => void;
    icon?: React.ReactNode;
    iconAfterText?: boolean;
    buttonType?: ButtonType;
    buttonDisabled?: boolean;
};

export function CardAction({
    type,
    onClick,
    subtitle,
    title,
    icon,
    iconAfterText,
    buttonType,
    buttonDisabled,
}: CardActionProps) {
    function handleActionClick(event: React.MouseEvent) {
        if (onClick) {
            event.stopPropagation();
            onClick();
        }
    }

    if (type === CardActionType.Link) {
        return (
            <div
                onClick={handleActionClick}
                className="card-action-link-color shrink-0 [&_svg]:h-5 [&_svg]:w-5"
            >
                {icon ? icon : <ArrowRight />}
            </div>
        );
    }

    if (type === CardActionType.SupportingText) {
        return (
            <div className="shrink-0 text-right">
                {title && (
                    <div className="card-action-title-color font-inter text-label-md">{title}</div>
                )}
                {subtitle && (
                    <div className="card-action-subtitle-color font-inter text-label-sm">
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
                    type={buttonType || ButtonType.Outlined}
                    size={ButtonSize.Small}
                    text={title}
                    onClick={handleActionClick}
                    icon={icon}
                    iconAfterText={iconAfterText}
                    disabled={buttonDisabled}
                />
            </div>
        );
    }

    return null;
}
