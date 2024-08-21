// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState, createContext, useContext, useRef, useEffect } from 'react';
import { TriangleDown } from '@iota/ui-icons';

const CollapseContext = createContext({
    isOpen: false,
    toggleOpen: () => {},
});

export function Collapse({ children }: { children: React.ReactNode }) {
    const [isOpen, setIsOpen] = useState(false);

    const toggleOpen = () => {
        setIsOpen(!isOpen);
    };

    return (
        <CollapseContext.Provider value={{ isOpen, toggleOpen }}>
            <div>{children}</div>
        </CollapseContext.Provider>
    );
}

export function CollapseHeader({ children, title }: { title: string; children: React.ReactNode }) {
    const { isOpen, toggleOpen } = useCollapse();

    return (
        <div
            onClick={toggleOpen}
            className="state-layer relative flex min-h-[52px] cursor-pointer items-center justify-between gap-1 rounded-md py-2 pl-1 pr-sm"
        >
            <div className="flex items-center gap-1">
                <TriangleDown
                    className={`${isOpen ? 'rotate-0 transition-transform ease-linear' : '-rotate-90 transition-transform ease-linear'} h-5 w-5 text-neutral-60`}
                />
                <div className="text-title-md">{title}</div>
            </div>
            {children}
        </div>
    );
}

export function CollapseBody({ children }: { children: React.ReactNode }) {
    const { isOpen } = useCollapse();
    const contentRef = useRef<HTMLDivElement>(null);
    const [height, setHeight] = useState(0);

    useEffect(() => {
        if (isOpen && contentRef.current) {
            setHeight(contentRef.current.scrollHeight);
        } else {
            setHeight(0);
        }
    }, [isOpen]);

    return (
        <div
            className="overflow-hidden px-4 transition-all duration-300 ease-in-out"
            ref={contentRef}
            style={{ maxHeight: `${height}px` }}
        >
            {isOpen ? children : null}
        </div>
    );
}

function useCollapse() {
    return useContext(CollapseContext);
}
