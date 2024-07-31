// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';

import { Button, ButtonSize, ButtonType } from '@/lib';
import { ArrowLeft, DoubleArrowLeft, ArrowRight } from '@iota/ui-icons';

export type TableProps = {
    /**
     * Does the table hava pagination buttons.
     */
    hasPagination?: boolean;
    /**
     * On Next page button click.
     */
    onNextPageClick?: () => void;
    /**
     * On Previous page button click.
     */
    onPreviousPageClick?: () => void;
    /**
     * On Previous page button click.
     */
    onFirstPageClick?: () => void;
    /**
     * The supporting label of the table.
     */
    supportingLabel?: string;
};

export function Table({
    hasPagination,
    onNextPageClick,
    onPreviousPageClick,
    onFirstPageClick,
    supportingLabel,
    children,
}: PropsWithChildren<TableProps>): JSX.Element {
    return (
        <div className="w-full">
            <div className="overflow-auto">
                <table className="w-full table-auto">{children}</table>
            </div>
            <div
                className={cx('flex w-full items-center justify-between pt-md', {
                    hidden: !supportingLabel && !hasPagination,
                })}
            >
                {hasPagination && (
                    <div className="flex gap-2">
                        <Button
                            type={ButtonType.Secondary}
                            size={ButtonSize.Small}
                            icon={<DoubleArrowLeft />}
                            onClick={onFirstPageClick}
                        />
                        <Button
                            type={ButtonType.Secondary}
                            size={ButtonSize.Small}
                            icon={<ArrowLeft />}
                            onClick={onPreviousPageClick}
                        />
                        <Button
                            type={ButtonType.Secondary}
                            size={ButtonSize.Small}
                            icon={<ArrowRight />}
                            onClick={onNextPageClick}
                        />
                    </div>
                )}
                {supportingLabel && (
                    <span className="ml-auto text-label-sm text-neutral-40 dark:text-neutral-60">
                        {supportingLabel}
                    </span>
                )}
            </div>
        </div>
    );
}

export function TableHeader({ children }: PropsWithChildren<object>): JSX.Element {
    return <thead>{React.Children.toArray(children)}</thead>;
}

export function TableRow({ children }: PropsWithChildren<object>): JSX.Element {
    return <tr>{children}</tr>;
}

export function TableBody({ children }: PropsWithChildren<object>): JSX.Element {
    return <tbody>{children}</tbody>;
}
