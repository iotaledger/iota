// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header } from '@iota/apps-ui-kit';
import { useCallback } from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';

interface PageTemplateProps {
    title?: string;
    children: ReactNode;
    onClose?: () => void;
    isTitleCentered?: boolean;
    showBackButton?: boolean;
}

export function PageTemplate({
    title,
    children,
    onClose,
    isTitleCentered,
    showBackButton,
}: PageTemplateProps) {
    const navigate = useNavigate();
    const handleBack = useCallback(() => navigate(-1), [navigate]);
    return (
        <div className="flex h-full w-full flex-col">
            {title && (
                <Header
                    titleCentered={isTitleCentered}
                    title={title}
                    onBack={showBackButton ? handleBack : undefined}
                    onClose={onClose}
                />
            )}
            <div className="w-full flex-1 overflow-hidden bg-neutral-100 p-md">{children}</div>
        </div>
    );
}
