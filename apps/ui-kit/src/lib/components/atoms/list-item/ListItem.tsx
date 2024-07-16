// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { ArrowRight } from '@iota/ui-icons';
import { Button, ButtonSize, ButtonType } from '@/components';

interface ListItemProps {
    showRightIcon?: boolean;
    /**
     * On right icon click handler (optional).
     */
    onRightIconClick?: () => void;
}

export function ListItem({
    showRightIcon,
    onRightIconClick,
    children,
}: PropsWithChildren<ListItemProps>): React.JSX.Element {
    return (
        <div className={cx('state-layer flex flex-row gap-1 p-xxs')}>
            {children}
            {showRightIcon && (
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onRightIconClick}
                    icon={<ArrowRight />}
                />
            )}
        </div>
    );
}
