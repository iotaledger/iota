// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';
import { PopupContext } from '@/contexts';
import React, { useState, ReactNode } from 'react';

interface PopupProviderProps {
    children: ReactNode;
}

function PopupProvider({ children }: PopupProviderProps): JSX.Element {
    const [popups, setPopups] = useState<ReactNode[]>([]);

    const openPopup = (content: ReactNode) => {
        setPopups((prevPopups) => [...prevPopups, content]);
    };

    const closePopup = () => {
        setPopups((prevPopups) => prevPopups.slice(0, prevPopups.length - 1));
    };

    return (
        <PopupContext.Provider value={{ popups, openPopup, closePopup }}>
            {children}
        </PopupContext.Provider>
    );
}

export default PopupProvider;
