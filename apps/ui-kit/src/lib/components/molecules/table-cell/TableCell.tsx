// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from 'react';
import { BadgeType, Badge, Checkbox, ButtonUnstyled } from '../../atoms';
import { TableCellType } from './table-cell.enums';
import { Copy } from '@iota/ui-icons';
import cx from 'classnames';
interface TableCellBaseProps {
    /**
     * The label of the cell.
     */
    label?: string;
    /**
     * If the cell is the last in the row and should not have a border.
     */
    hasLastBorderNoneClass?: boolean;
    /**
     * Whether the cell content should be centered.
     */
    isContentCentered?: boolean;
}

type TableCellText = {
    /**
     * The type of the cell.
     */
    type: TableCellType.Text;
    /**
     * The supporting label of the cell.
     */
    supportingLabel?: string;
};

type TableCellTextToCopy = {
    /**
     * The type of the cell.
     */
    type: TableCellType.TextToCopy;
    /**
     * The text to be copied.
     */
    textToCopy: string;
    /**
     * The onCopySuccess event of the Address  (optional).
     */
    onCopySuccess?: (e: React.MouseEvent<HTMLButtonElement>, text: string) => void;
    /**
     * The onCopyError event of the Address  (optional).
     */
    onCopyError?: (e: unknown, text: string) => void;
};

type TableCellBadge = {
    /**
     * The type of the cell.
     */
    type: TableCellType.Badge;
    /**
     * The type of the badge
     */
    badgeType: BadgeType;
};

type TableCellAvatarText = {
    /**
     * The type of the cell.
     */
    type: TableCellType.AvatarText;
    /**
     * The leading element of the cell.
     */
    leadingElement: React.JSX.Element;
};

type TableCellCheckbox = {
    /**
     * The type of the cell.
     */
    type: TableCellType.Checkbox;
    /**
     * The state of the checkbox.
     */
    isChecked?: boolean;
    /**
     * The function to call when the checkbox is clicked.
     */
    onChange?: (e: React.ChangeEvent<HTMLInputElement>) => void;
    /**
     * If true the checkbox will override the styles to show an indeterminate state.
     */
    isIndeterminate?: boolean;
};

type TableCellPlaceholder = {
    /**
     * The type of the cell.
     */
    type: TableCellType.Placeholder;
};

type TableCellLink = {
    /**
     * The type of the cell.
     */
    type: TableCellType.Link;
    /**
     * The link to navigate to.
     */
    to: string;
    /**
     * If true the link will open in a new tab.
     */
    isExternal?: boolean;
};

export type TableCellProps = TableCellBaseProps &
    (
        | TableCellText
        | TableCellTextToCopy
        | TableCellBadge
        | TableCellAvatarText
        | TableCellCheckbox
        | TableCellPlaceholder
        | TableCellLink
    );

export function TableCell(props: TableCellProps): JSX.Element {
    const { type, label, hasLastBorderNoneClass, isContentCentered } = props;

    const textColorClass = 'text-neutral-40 dark:text-neutral-60';
    const textSizeClass = 'text-body-md';

    async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement, MouseEvent>) {
        if (props.type === TableCellType.TextToCopy) {
            try {
                await navigator.clipboard.writeText(props.textToCopy);
                props.onCopySuccess?.(event, props.textToCopy);
            } catch (error) {
                console.error('Failed to copy:', error);
                props.onCopyError?.(error, props.textToCopy);
            }
        }
    }

    const Cell = () => {
        switch (type) {
            case TableCellType.Text:
                const { supportingLabel } = props;
                return (
                    <div className="flex flex-row items-baseline gap-1">
                        <span className={cx(textColorClass, textSizeClass)}>{label}</span>
                        {supportingLabel && (
                            <span className="text-body-sm text-neutral-60 dark:text-neutral-40">
                                {supportingLabel}
                            </span>
                        )}
                    </div>
                );
            case TableCellType.TextToCopy:
                return (
                    <div
                        className={cx(
                            'flex items-center space-x-2 [&_svg]:h-4 [&_svg]:w-4',
                            textColorClass,
                            textSizeClass,
                        )}
                    >
                        <span>{label}</span>
                        <ButtonUnstyled onClick={handleCopyClick}>
                            <Copy />
                        </ButtonUnstyled>
                    </div>
                );
            case TableCellType.Badge:
                const { badgeType } = props;
                return <Badge type={badgeType} label={label} />;
            case TableCellType.AvatarText:
                const { leadingElement } = props;
                return (
                    <div className={cx('flex items-center gap-x-2.5', textColorClass)}>
                        {leadingElement}
                        <span className="text-label-lg">{label}</span>
                    </div>
                );
            case TableCellType.Checkbox:
                const { isChecked, onChange, isIndeterminate } = props;
                return (
                    <Checkbox
                        isChecked={isChecked}
                        onCheckedChange={onChange}
                        isIndeterminate={isIndeterminate}
                    />
                );
            case TableCellType.Placeholder:
                return (
                    <div className="h-[1em] w-full animate-shimmer rounded-md bg-placeholderShimmer bg-[length:1000px_100%] dark:bg-placeholderShimmerDark"></div>
                );

            case TableCellType.Link:
                const { to, isExternal } = props;
                return (
                    <a
                        href={to}
                        target={isExternal ? '_blank' : '_self'}
                        rel="noopener noreferrer"
                        className={cx('text-primary-30 dark:text-primary-80', textSizeClass)}
                    >
                        {label}
                    </a>
                );
            default:
                return null;
        }
    };

    return (
        <td
            className={cx(
                'h-14 border-b border-shader-neutral-light-8 px-md dark:border-shader-neutral-dark-8',
                { 'last:border-none': hasLastBorderNoneClass },
                { 'flex items-center justify-center': isContentCentered },
            )}
        >
            <Cell />
        </td>
    );
}
