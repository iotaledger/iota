// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRef, useState, useLayoutEffect } from 'react';
import type { PropsWithChildren } from 'react';
import { createPortal } from 'react-dom';
import cx from 'classnames';
import { TooltipPosition } from './tooltip.enums';

interface TooltipProps {
    text: string;
    position?: TooltipPosition;
    maxWidth?: string;
    offset?: number;
    openDelay?: number;
    closeDelay?: number;
}

export function Tooltip({
    text,
    position = TooltipPosition.Top,
    maxWidth = 'max-w-[200px]',
    offset = 8,
    openDelay = 0,
    closeDelay = 100,
    children,
}: PropsWithChildren<TooltipProps>) {
    const triggerRef = useRef<HTMLDivElement>(null);
    const tooltipRef = useRef<HTMLDivElement>(null);

    const [visible, setVisible] = useState(false);
    const [coords, setCoords] = useState<{ top: number; left: number }>({ top: 0, left: 0 });

    const openTimer = useRef<ReturnType<typeof setTimeout>>();
    const closeTimer = useRef<ReturnType<typeof setTimeout>>();

    const clearTimers = () => {
        clearTimeout(openTimer.current);
        clearTimeout(closeTimer.current);
    };

    const open = () => {
        clearTimers();
        openTimer.current = setTimeout(() => setVisible(true), openDelay);
    };

    const close = () => {
        clearTimers();
        closeTimer.current = setTimeout(() => setVisible(false), closeDelay);
    };

    useLayoutEffect(() => {
        if (!visible) return;

        const rect = triggerRef.current?.getBoundingClientRect();
        if (!rect) return;

        const pos = {
            [TooltipPosition.Top]: {
                top: rect.top - offset,
                left: rect.left + rect.width / 2,
                transform: 'translate(-50%, -100%)',
            },
            [TooltipPosition.Bottom]: {
                top: rect.bottom + offset,
                left: rect.left + rect.width / 2,
                transform: 'translate(-50%, 0)',
            },
            [TooltipPosition.Left]: {
                top: rect.top + rect.height / 2,
                left: rect.left - offset,
                transform: 'translate(-100%, -50%)',
            },
            [TooltipPosition.Right]: {
                top: rect.top + rect.height / 2,
                left: rect.right + offset,
                transform: 'translate(0, -50%)',
            },
        }[position];

        setCoords({ top: pos.top, left: pos.left });
        tooltipRef.current!.style.transform = pos.transform;
    }, [visible, position, offset]);

    useLayoutEffect(() => {
        if (!visible) return;
        const update = () => {
            const rect = triggerRef.current?.getBoundingClientRect();
            if (!rect) return;
            setCoords((prev) => ({ ...prev, left: rect.left + rect.width / 2 }));
        };
        window.addEventListener('scroll', update, true);
        window.addEventListener('resize', update);
        return () => {
            window.removeEventListener('scroll', update, true);
            window.removeEventListener('resize', update);
        };
    }, [visible]);

    // z-[9999999999]: needed because we must exceed the popup’s ≈2 147 483 647 z-index;
    // otherwise the tooltip renders but stays invisible inside a Chrome extension
    const base = 'z-[9999999999] w-max rounded p-xs tooltip-bg tooltip-text-color';

    return (
        <>
            <div
                ref={triggerRef}
                className="inline-block cursor-pointer"
                onMouseEnter={open}
                onFocus={open}
                onMouseLeave={close}
                onBlur={close}
            >
                {children}
            </div>

            {visible &&
                createPortal(
                    <div
                        ref={tooltipRef}
                        role="tooltip"
                        style={{
                            position: 'fixed',
                            top: coords.top,
                            left: coords.left,
                            transition: 'opacity .15s ease',
                            opacity: 1,
                        }}
                        className={cx(base, maxWidth)}
                        onMouseEnter={open}
                        onMouseLeave={close}
                    >
                        <p className="w-full break-words">{text}</p>
                    </div>,
                    document.body,
                )}
        </>
    );
}
