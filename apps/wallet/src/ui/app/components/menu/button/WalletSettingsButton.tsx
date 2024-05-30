// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonOrLink } from '_src/ui/app/shared/utils/ButtonOrLink';
import { HamburgerOpen24 as HamburgerOpenIcon, Settings24 as SettingsIcon } from '@mysten/icons';
import { cx } from 'class-variance-authority';

import { useMenuIsOpen, useNextMenuUrl } from '../hooks';

export function WalletSettingsButton() {
    const isOpen = useMenuIsOpen();
    const menuUrl = useNextMenuUrl(!isOpen, '/');
    const IconComponent = isOpen ? HamburgerOpenIcon : SettingsIcon;

    return (
        <ButtonOrLink
            className={cx(
                'ml-auto flex cursor-pointer appearance-none items-center justify-center border-none bg-transparent hover:text-hero-dark',
                { 'text-steel': !isOpen, 'text-gray-90': isOpen },
            )}
            aria-label={isOpen ? 'Close settings menu' : 'Open settings menu'}
            to={menuUrl}
        >
            <IconComponent className="h-6 w-6" />
        </ButtonOrLink>
    );
}
