// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';
import { Close, Settings } from '@iota/apps-ui-icons';
import { useMenuIsOpen, useNextMenuUrl } from '_components';
import { Link } from 'react-router-dom';
import { cx } from 'class-variance-authority';

export function WalletSettingsButton() {
    const isOpen = useMenuIsOpen();
    const menuUrl = useNextMenuUrl(!isOpen, '/');
    const IconComponent = isOpen ? Close : Settings;

    return (
        <Link
            className={cx('flex cursor-pointer')}
            aria-label={isOpen ? 'Close settings menu' : 'Open settings menu'}
            to={menuUrl}
        >
            <Button
                type={ButtonType.Ghost}
                size={ButtonSize.Small}
                icon={<IconComponent className="h-5 w-5" />}
            />
        </Link>
    );
}
