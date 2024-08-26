// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import type { ReactNode } from 'react';
import { Header } from '@iota/apps-ui-kit';
import { Portal } from '../shared/Portal';
import { useNavigate } from 'react-router-dom';

interface OverlayProps {
    title?: string;
    children: ReactNode;
    showModal: boolean;
    closeOverlay?: () => void;
    closeIcon?: ReactNode | null;
    setShowModal?: (showModal: boolean) => void;
    background?: 'bg-iota-lightest';
    showBackButton?: boolean;
    noContentPadding?: boolean;
    actionBar?: ReactNode;
}

export function Overlay({
    title,
    children,
    showModal,
    closeOverlay,
    showBackButton,
    setShowModal,
    noContentPadding,
    actionBar,
}: OverlayProps) {
    const closeModal = useCallback(
        (e: React.MouseEvent<HTMLElement>) => {
            closeOverlay && closeOverlay();
            setShowModal && setShowModal(false);
        },
        [closeOverlay, setShowModal],
    );
    const navigate = useNavigate();
    const handleBack = useCallback(() => navigate(-1), [navigate]);
    return showModal ? (
        <Portal containerId="overlay-portal-container">
            <div className="absolute inset-0 z-[9999] flex flex-col flex-nowrap items-center backdrop-blur-[20px]">
                {title && (
                    <Header
                        onBack={showBackButton ? handleBack : undefined}
                        titleCentered
                        title={title}
                        onClose={closeModal}
                    />
                )}
                <div
                    className={`w-full flex-1 overflow-hidden bg-neutral-100 ${noContentPadding ? '' : 'p-md'}`}
                >
                    {children}
                </div>
                {actionBar && <div className="w-full bg-white px-md pb-md pt-sm">{actionBar}</div>}
            </div>
        </Portal>
    ) : null;
}
