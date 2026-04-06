// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonUnstyled } from '../../atoms/button';
import { MoreHoriz } from '@iota/apps-ui-icons';
import cx from 'classnames';
import { AssetMediaRenderer } from '../../molecules/asset-media-renderer';
import type { AssetMediaRendererProps } from '../../molecules/asset-media-renderer';

interface VisualAssetBaseProps extends React.AriaAttributes {
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

interface CustomAssetDisplayProps {
    /**
     * The media to be displayed inside the card.
     */
    renderAsset?: React.ReactNode;
}

export type VisualAssetCardProps = VisualAssetBaseProps &
    (AssetMediaRendererProps | CustomAssetDisplayProps);

export function VisualAssetCard({
    onIconClick,
    onClick,
    icon = <MoreHoriz />,
    assetTitle,
    isHoverable = true,
    ...assetProps
}: VisualAssetBaseProps & (AssetMediaRendererProps | CustomAssetDisplayProps)): React.JSX.Element {
    const handleIconClick = (event: React.MouseEvent<HTMLButtonElement>) => {
        onIconClick?.(event);
        event?.stopPropagation();
    };

    const isCustomAsset = !('src' in assetProps);

    return (
        <div
            className={cx('relative aspect-square w-full overflow-hidden rounded-xl', {
                'group cursor-pointer': isHoverable,
            })}
            onClick={onClick}
        >
            {!isCustomAsset && <AssetMediaRenderer {...assetProps} />}

            {isCustomAsset && assetProps.renderAsset}

            {isHoverable && (
                <div className="absolute left-0 top-0 h-full w-full bg-cover bg-center bg-no-repeat group-hover:bg-shader-neutral-light-48 group-hover:transition group-hover:duration-300 group-hover:ease-in-out group-hover:names:bg-shader-inverted-light-48 group-hover:dark:bg-shader-primary-dark-48" />
            )}
            {isHoverable && onIconClick && (
                <ButtonUnstyled
                    className="visual-asset-icon-color absolute right-2 top-2 z-10 h-9 w-9 cursor-pointer rounded-full p-xs opacity-0 transition-opacity duration-300 group-hover:bg-shader-neutral-light-72 group-hover:opacity-100 [&_svg]:h-5 [&_svg]:w-5"
                    onClick={handleIconClick}
                    aria-label="More options"
                >
                    {icon}
                </ButtonUnstyled>
            )}
            {isHoverable && assetTitle && (
                <div className="absolute bottom-0 w-full p-xs opacity-0 transition-opacity duration-300 group-hover:opacity-100">
                    <span className="visual-asset-title-color break-words text-title-md">
                        {assetTitle}
                    </span>
                </div>
            )}
        </div>
    );
}
