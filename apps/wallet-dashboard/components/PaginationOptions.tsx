// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonType, ButtonSize } from '@iota/apps-ui-kit';
import { ArrowLeft, ArrowRight, DoubleArrowLeft, DoubleArrowRight } from '@iota/apps-ui-icons';

interface PaginationOptionsProps {
    pagination: {
        onFirst?: () => void;
        hasFirst?: boolean;
        onPrev?: () => void;
        hasPrev?: boolean;
        onNext?: () => void;
        hasNext?: boolean;
        onLast?: () => void;
        hasLast?: boolean;
    };
    action?: React.ReactNode;
}

export function PaginationOptions({
    pagination: { onFirst, hasFirst, onPrev, hasPrev, onNext, hasNext, onLast, hasLast },
    action,
}: PaginationOptionsProps): React.JSX.Element {
    const PAGINATION_BUTTONS: React.ComponentProps<typeof Button>[] = [
        {
            icon: <DoubleArrowLeft />,
            disabled: !hasFirst,
            onClick: onFirst,
        },
        {
            icon: <ArrowLeft />,
            disabled: !hasPrev,
            onClick: onPrev,
        },
        {
            icon: <ArrowRight />,
            disabled: !hasNext,
            onClick: onNext,
        },
        {
            icon: <DoubleArrowRight />,
            disabled: !hasLast,
            onClick: onLast,
        },
    ];
    return (
        <div className="flex gap-2">
            <div className="flex flex-row items-center gap-2">
                {PAGINATION_BUTTONS.map((button, index) => (
                    <Button
                        key={index}
                        type={ButtonType.Secondary}
                        size={ButtonSize.Small}
                        {...button}
                    />
                ))}
            </div>
            {action}
        </div>
    );
}
