// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ComponentProps } from 'react';
import { Button, Tooltip, TooltipPosition } from '../../atoms';
import { Info } from '@iota/ui-icons';

interface TitleProps {
    /**
     * The title of the component.
     */
    title: string;
    /**
     * The subtitle of the component.
     */
    subtitle?: string;
    /**
     * Props for the button component.
     */
    button?: ComponentProps<typeof Button>;
    /**
     * The tooltip position.
     */
    tooltipPosition?: TooltipPosition;
    /**
     * The tooltip text.
     */
    tooltipText?: string;
}

export function Title({ title, subtitle, button, tooltipText, tooltipPosition }: TitleProps) {
    return (
        <div className="flex flex-row items-center justify-between gap-x-6 px-md py-sm">
            <div className="flex flex-col justify-start">
                <div className="flex flex-row items-center gap-x-0.5 text-neutral-10 dark:text-neutral-92">
                    <h4 className="text-title-lg">{title}</h4>
                    {tooltipText && (
                        <Tooltip text={tooltipText} position={tooltipPosition}>
                            <Info />
                        </Tooltip>
                    )}
                </div>
                <p className="text-label-md text-neutral-60 dark:text-neutral-40">{subtitle}</p>
            </div>
            <Button {...button} />
        </div>
    );
}
