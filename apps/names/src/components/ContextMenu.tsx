// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dropdown, ListItem } from '@iota/apps-ui-kit';
import { useRef } from 'react';
import { createPortal } from 'react-dom';

import { MenuListItem } from '@/lib/types/components';

interface ContextMenuProps {
    visible: boolean;
    position: { top: number; left: number };
    options: MenuListItem[];
    dropdownRef?: (el: HTMLDivElement | null) => void;
}

export function ContextMenuDropdown({ visible, position, options, dropdownRef }: ContextMenuProps) {
    const lastSent = useRef<HTMLDivElement | null>(null);
    if (!visible) return null;

    const positionContextMenu = (node: HTMLDivElement | null) => {
        if (dropdownRef && lastSent.current !== node) {
            dropdownRef(node);
            lastSent.current = node;
        }
        if (!node) return;

        const approxHeight = 320;
        const margin = 8;
        const GAP_UP = 50;
        const { innerHeight } = window;

        const isBelowVisibleArea = position.top + approxHeight + margin <= innerHeight;

        node.style.top = '';
        node.style.bottom = '';
        node.style.left = `${position.left}px`;

        if (isBelowVisibleArea) {
            node.style.top = `${position.top + margin}px`;
        } else {
            const bottom = innerHeight - position.top + margin + GAP_UP;
            node.style.bottom = `${bottom}px`;
        }
    };

    return createPortal(
        <div
            ref={positionContextMenu}
            onMouseDown={(e) => {
                e.stopPropagation();
            }}
            onClick={(e) => {
                e.stopPropagation();
            }}
            className="fixed z-[110000] pointer-events-auto max-h-[calc(100vh-16px)] overflow-y-auto left-1/2 -translate-x-1/2 max-w-[92vw]"
        >
            <Dropdown>
                {options
                    .filter((option) => !option.isHidden)
                    .map((item, index) => (
                        <ListItem key={index} {...item} />
                    ))}
            </Dropdown>
        </div>,
        document.body,
    );
}
