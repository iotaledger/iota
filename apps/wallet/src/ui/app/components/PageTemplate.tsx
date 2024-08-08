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

function PageTemplate({
    title,
    children,
    onClose: closePageTemplate,
    isTitleCentered,
    showBackButton,
}: PageTemplateProps) {
    const closeModal = useCallback(() => {
        closePageTemplate && closePageTemplate();
    }, [closePageTemplate]);
    const navigate = useNavigate();
    const handleBack = useCallback(() => navigate(-1), [navigate]);
    return (
        <div className="h-full w-full">
            {title && (
                <Header
                    titleCentered={isTitleCentered}
                    title={title}
                    onBack={showBackButton ? handleBack : undefined}
                    onClose={closeModal}
                />
            )}
            <div className="h-full w-full bg-white p-md">{children}</div>
        </div>
    );
}

export default PageTemplate;
