// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import type { InfoBoxType } from './infoBox.enums';
import { InfoBoxStyle } from './infoBox.enums';
import { BACKGROUND_COLORS, ICON_COLORS } from './infoBox.classes';

export interface InfoBoxProps {
    /**
     * The icon of the info box (optional).
     */
    icon?: React.ReactNode;
    /**
     * The title of the info box (optional).
     */
    title?: string;
    /**
     * The supporting text of the info box (optional).
     */
    supportingText?: React.ReactNode;
    /**
     * The type of the info box.
     */
    type: InfoBoxType;
    /**
     * The style of the info box.
     */
    style?: InfoBoxStyle;
}

export function InfoBox({
    icon,
    title,
    supportingText,
    type,
    style,
}: InfoBoxProps): React.JSX.Element {
    const iconColorClass = ICON_COLORS[type];
    const backgroundClass = style === InfoBoxStyle.Elevated ? BACKGROUND_COLORS[type] : '';
    return (
        <div
            className={cx('flex flex-row items-start gap-4 py-xs pr-lg', backgroundClass, {
                'rounded-lg pl-xs': style === InfoBoxStyle.Elevated,
            })}
        >
            {icon && (
                <span
                    className={cx(
                        'flex items-center justify-center rounded-lg [&_svg]:h-4 [&_svg]:w-4',
                        iconColorClass,
                        {
                            'p-xs': style === InfoBoxStyle.Default,
                        },
                    )}
                >
                    {icon}
                </span>
            )}
            <div className="flex flex-col gap-1">
                {title && <span className="infobox-text-title text-title-sm">{title}</span>}
                {supportingText && (
                    <span className="infobox-supporting-text text-body-sm">{supportingText}</span>
                )}
            </div>
        </div>
    );
}
