// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { useNameContextMenu } from '@/hooks/useNameContextMenu';
import { MenuListItem } from '@/lib/types/components';

import { ContextMenuDropdown } from '../ContextMenu';
import { MenuButton } from './MenuButton';

interface ContextMenuButtonProps {
    options: MenuListItem[];
}

export function ContextMenuButton({ options }: ContextMenuButtonProps) {
    const { isVisible, position, toggleMenu, dropdownRef, triggerRef } = useNameContextMenu();

    return (
        <>
            <MenuButton onClick={toggleMenu} ref={triggerRef} />

            <ContextMenuDropdown
                visible={isVisible}
                position={position}
                options={options.map((option) => ({
                    ...option,
                    onClick: () => {
                        option.onClick?.();
                        toggleMenu();
                    },
                }))}
                dropdownRef={dropdownRef}
            />
        </>
    );
}
