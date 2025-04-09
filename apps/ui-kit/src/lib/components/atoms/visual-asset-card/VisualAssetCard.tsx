// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetType } from './visualAssetCard.enums';
import { ButtonUnstyled } from '../button';
import { MoreHoriz, PlaceholderReplace } from '@iota/apps-ui-icons';
import cx from 'classnames';
import { useState } from 'react';

export interface VisualAssetCardProps {
    /**
     * The type of the asset to be displayed.
     */
    assetType?: VisualAssetType;
    /**
     * The source of the image to be displayed.
     */
    assetSrc: string;
    /**
     * Alt text for the image.
     */
    altText: string;
    /**
     * The onClick event for the icon.
     */
    onIconClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The onClick event for the card.
     */
    onClick?: (e: React.MouseEvent<HTMLDivElement>) => void;
    /**
     * The icon to be displayed.
     */
    icon?: React.ReactNode;
    /**
     * The title text to be displayed on hover.
     */
    assetTitle?: string;
    /**
     * Whether the card is hoverable.
     */
    isHoverable?: boolean;
}

export function VisualAssetCard({
    assetType = VisualAssetType.Image,
    assetSrc,
    altText,
    onIconClick,
    onClick,
    icon = <MoreHoriz />,
    assetTitle,
    isHoverable = true,
}: VisualAssetCardProps): React.JSX.Element {
    const [imageError, setImageError] = useState(false);

    const handleIconClick = (event: React.MouseEvent<HTMLButtonElement>) => {
        onIconClick?.(event);
        event?.stopPropagation();
    };

    return (
        <div
            className={cx('relative aspect-square w-full overflow-hidden rounded-xl', {
                'group cursor-pointer': isHoverable,
            })}
            onClick={onClick}
        >
            {assetType === VisualAssetType.Video ? (
                <video src={assetSrc} className="h-full w-full object-cover" autoPlay loop muted />
            ) : imageError ? (
                <div className="flex h-full w-full items-center justify-center bg-neutral-96 dark:bg-neutral-10">
                    <PlaceholderReplace className="h-4 w-4 text-neutral-40 dark:text-neutral-60" />
                </div>
            ) : (
                <img
                    src={assetSrc}
                    alt={altText}
                    className="h-full w-full object-cover"
                    onError={() => setImageError(true)}
                />
            )}
            {isHoverable && (
                <div className="absolute left-0 top-0 h-full w-full bg-cover bg-center bg-no-repeat group-hover:bg-shader-neutral-light-48 group-hover:transition group-hover:duration-300 group-hover:ease-in-out group-hover:dark:bg-shader-primary-dark-48" />
            )}
            {isHoverable && onIconClick && (
                <ButtonUnstyled
                    className="absolute right-2 top-2 h-9 w-9 cursor-pointer rounded-full p-xs opacity-0 transition-opacity duration-300 group-hover:bg-shader-neutral-light-72 group-hover:opacity-100 [&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-primary-100"
                    onClick={handleIconClick}
                >
                    {icon}
                </ButtonUnstyled>
            )}
            {isHoverable && assetTitle && (
                <div className="absolute bottom-0 flex items-center justify-center p-xs opacity-0 transition-opacity duration-300 group-hover:opacity-100">
                    <span className="text-title-md text-neutral-100">{assetTitle}</span>
                </div>
            )}
        </div>
    );
}
