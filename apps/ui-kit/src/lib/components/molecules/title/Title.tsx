// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { TooltipPosition } from '@/components/atoms';
import { Tooltip } from '@/components/atoms';
import { Info } from '@iota/apps-ui-icons';
import { TitleSize } from './titleSize.enums';
import cx from 'classnames';
import { TITLE_PADDINGS, TITLE_SIZE } from './titleClasses.constants';

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
     * The trailing element.
     */
    trailingElement?: React.ReactNode;
    /**
     * The tooltip position.
     */
    tooltipPosition?: TooltipPosition;
    /**
     * The tooltip text.
     */
    tooltipText?: string;
    /**
     * Supporting Element
     */
    supportingElement?: React.ReactNode;
    /**
     * The size of the component
     */
    size?: TitleSize;
    /**
     * The 'data-testid' attribute value (used in e2e tests)
     */
    testId?: string;
}

export function Title({
    title,
    subtitle,
    trailingElement,
    tooltipText,
    supportingElement,
    tooltipPosition,
    size = TitleSize.Medium,
    testId,
}: TitleProps) {
    return (
        <div
            className={cx('flex flex-row items-center justify-between', TITLE_PADDINGS[size])}
            data-testid={testId}
        >
            <div className="flex flex-row items-center gap-x-xxxs">
                <div className="flex flex-col justify-start">
                    <div className="flex flex-row items-center gap-x-0.5 text-neutral-10 dark:text-neutral-92">
                        <h4 className={cx(TITLE_SIZE[size])}>{title}</h4>
                        {tooltipText && (
                            <Tooltip text={tooltipText} position={tooltipPosition}>
                                <Info />
                            </Tooltip>
                        )}
                    </div>
                    <p className="text-label-md text-neutral-60 dark:text-neutral-40">{subtitle}</p>
                </div>
                {supportingElement}
            </div>
            {trailingElement}
        </div>
    );
}
