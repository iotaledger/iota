// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header } from '@iota/apps-ui-kit';
import { useCallback } from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';

interface PageTemplateProps {
    title?: string;
    children: ReactNode;
    closePageTemplate?: () => void;
    isTitleCentered?: boolean;
    displayBackButton?: boolean;
}

function PageTemplate({
    title,
    children,
    closePageTemplate,
    isTitleCentered,
    displayBackButton,
}: PageTemplateProps) {
    const closeModal = useCallback(() => {
        closePageTemplate && closePageTemplate();
    }, [closePageTemplate]);
    const navigate = useNavigate();

    return (
        <div className="h-full w-full">
            {title && (
                <Header
                    titleCentered={isTitleCentered}
                    title={title}
                    onBack={displayBackButton ? () => navigate(-1) : undefined}
                    onClose={closeModal}
                />
            )}
            <div className="h-full w-full bg-white p-md">{children}</div>
        </div>
    );
}

export default PageTemplate;
