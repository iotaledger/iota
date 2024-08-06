// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';

import { Button, ButtonSize, ButtonType } from '@/lib';
import { ArrowLeft, DoubleArrowLeft, ArrowRight, DoubleArrowRight } from '@iota/ui-icons';

export type TableProps = {
    /**
     * Does the table have pagination buttons.
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
     * On First page button click.
     */
    onFirstPageClick?: () => void;
    /**
     * On Last page button click.
     */
    onLastPageClick?: () => void;
    /**
     * The label of the action button.
     */
    actionLabel?: string;
    /**
     * On Action button click.
     */
    onActionClick?: () => void;
    /**
     * The supporting label of the table.
     */
    supportingLabel?: string;
};

export function Table({
    hasPagination,
    actionLabel,
    onNextPageClick,
    onPreviousPageClick,
    onFirstPageClick,
    onLastPageClick,
    onActionClick,
    supportingLabel,
    children,
}: PropsWithChildren<TableProps>): JSX.Element {
    return (
        <div className="w-full">
            <div className="overflow-auto">
                <table className="w-full table-auto">{children}</table>
            </div>
            <div
                className={cx('flex w-full items-center justify-between gap-2 pt-md', {
                    hidden: !supportingLabel && !hasPagination && !actionLabel,
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
                        <Button
                            type={ButtonType.Secondary}
                            size={ButtonSize.Small}
                            icon={<DoubleArrowRight />}
                            onClick={onLastPageClick}
                        />
                    </div>
                )}
                {actionLabel && (
                    <div className="flex">
                        <Button
                            type={ButtonType.Secondary}
                            size={ButtonSize.Small}
                            text={actionLabel}
                            onClick={onActionClick}
                        />
                    </div>
                )}
                {supportingLabel && (
                    <span className="ml-auto text-label-md text-neutral-40 dark:text-neutral-60">
                        {supportingLabel}
                    </span>
                )}
            </div>
        </div>
    );
}

export function TableHeader({ children }: PropsWithChildren): JSX.Element {
    return <thead>{children}</thead>;
}

export function TableRow({ children }: PropsWithChildren): JSX.Element {
    return <tr>{children}</tr>;
}

export function TableBody({ children }: PropsWithChildren): JSX.Element {
    return <tbody>{children}</tbody>;
}
