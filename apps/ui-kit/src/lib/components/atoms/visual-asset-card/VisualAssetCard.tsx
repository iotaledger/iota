// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

export interface VisualAssetCardProps {
    /**
     * The source of the image to be displayed.
     */
    imageSrc: string;
    /**
     * Alt text for the image.
     */
    altText: string;
    /**
     * The onClick event for the icon.
     */
    onIconClick?: (e: React.MouseEvent<HTMLDivElement>) => void;
    /**
     * The icon to be displayed.
     */
    icon: React.ReactNode;
}

export function VisualAssetCard({
    imageSrc,
    altText,
    onIconClick,
    icon,
}: VisualAssetCardProps): React.JSX.Element {
    return (
        <div className="group relative aspect-square w-full overflow-hidden rounded-xl hover:cursor-pointer">
            <img src={imageSrc} alt={altText} className="h-full w-full object-cover" />
            <div className="absolute left-0 top-0 h-full w-full bg-cover bg-center bg-no-repeat group-hover:bg-shader-neutral-light-48 group-hover:transition group-hover:duration-300 group-hover:ease-in-out" />
            <div
                className="absolute right-2 top-2 h-9 w-9 cursor-pointer rounded-full p-xs opacity-0 transition-opacity duration-300 group-hover:bg-shader-neutral-light-72 group-hover:opacity-100 [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-primary-100"
                onClick={onIconClick}
            >
                {icon}
            </div>
        </div>
    );
}
