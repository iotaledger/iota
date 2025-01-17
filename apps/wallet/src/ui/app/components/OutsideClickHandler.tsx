// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from 'react';

interface OutsideClickHandlerProps {
    onOutsideClick: () => void;
    children: React.ReactNode;
}

export const OutsideClickHandler: React.FC<OutsideClickHandlerProps> = ({
    onOutsideClick,
    children,
}) => {
    const ref = useRef<HTMLDivElement>(null);

    useEffect(() => {
        function handleClick(event: MouseEvent) {
            if (ref.current && !ref.current.contains(event.target as Node)) {
                onOutsideClick();
            }
        }
        document.addEventListener('mousedown', handleClick);
        return () => {
            document.removeEventListener('mousedown', handleClick);
        };
    }, [onOutsideClick]);

    return <div ref={ref}>{children}</div>;
};
