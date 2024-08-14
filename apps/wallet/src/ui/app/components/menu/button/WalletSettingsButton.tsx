// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, Settings } from '@iota/ui-icons';
import { useMenuIsOpen, useNextMenuUrl } from '_components';
import { ButtonOrLink } from '_src/ui/app/shared/utils/ButtonOrLink';
import { cx } from 'class-variance-authority';

export function WalletSettingsButton() {
    const isOpen = useMenuIsOpen();
    const menuUrl = useNextMenuUrl(!isOpen, '/');
    const IconComponent = isOpen ? Close : Settings;

    return (
        <ButtonOrLink
            className={cx(
                'hover:text-hero-dark ml-auto flex cursor-pointer appearance-none items-center justify-center border-none bg-transparent p-xs text-neutral-10 [&_svg]:h-5 [&_svg]:w-5',
                { 'text-steel': !isOpen, 'text-gray-90': isOpen },
            )}
            aria-label={isOpen ? 'Close settings menu' : 'Open settings menu'}
            to={menuUrl}
        >
            <IconComponent />
        </ButtonOrLink>
    );
}
