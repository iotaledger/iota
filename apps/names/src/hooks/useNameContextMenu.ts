// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from 'react';

interface UseNameContextMenuReturn {
    isVisible: boolean;
    position: { top: number; left: number };
    openMenu: () => void;
    closeMenu: () => void;
    toggleMenu: () => void;
    triggerRef: (el: HTMLElement | null) => void;
    dropdownRef: (el: HTMLDivElement | null) => void;
}
export function useNameContextMenu(): UseNameContextMenuReturn {
    const triggerRef = useRef<HTMLElement | null>(null);
    const dropdownRef = useRef<HTMLDivElement | null>(null);
    const [isVisible, setIsVisible] = useState(false);
    const [position, setPosition] = useState({ top: 0, left: 0 });

    const openMenu = useCallback(() => {
        if (!triggerRef.current) return;

        const rect = triggerRef.current.getBoundingClientRect();
        const top = rect.bottom + 8;
        const left = Math.min(rect.left, window.innerWidth - 200);

        setPosition({ top, left });
        setIsVisible(true);
    }, []);

    const registerTriggerRef = useCallback((el: HTMLElement | null) => {
        triggerRef.current = el;
    }, []);

    const registerDropdownRef = useCallback((el: HTMLDivElement | null) => {
        dropdownRef.current = el;
    }, []);

    const closeMenu = useCallback(() => setIsVisible(false), []);

    useEffect(() => {
        if (!isVisible) return;

        const handleClickOutside = (e: MouseEvent) => {
            const target = e.target as Node;
            if (
                dropdownRef.current &&
                !dropdownRef.current.contains(target) &&
                triggerRef.current &&
                !triggerRef.current.contains(target)
            ) {
                closeMenu();
            }
        };

        const handleResize = () => {
            if (triggerRef.current) {
                const rect = triggerRef.current.getBoundingClientRect();
                setPosition({ top: rect.bottom + 8, left: rect.left });
            }
        };

        window.addEventListener('resize', handleResize);
        window.addEventListener('scroll', handleResize, true);
        document.addEventListener('mousedown', handleClickOutside);

        return () => {
            window.removeEventListener('resize', handleResize);
            window.removeEventListener('scroll', handleResize, true);
            document.removeEventListener('mousedown', handleClickOutside);
        };
    }, [isVisible, closeMenu]);

    return {
        isVisible,
        position,
        openMenu,
        closeMenu,
        toggleMenu: isVisible ? closeMenu : openMenu,
        triggerRef: registerTriggerRef,
        dropdownRef: registerDropdownRef,
    };
}
