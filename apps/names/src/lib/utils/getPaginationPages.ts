// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const SEPARATOR = '...';

export function getPaginationPages(
    currentPage: number,
    lastPage: number,
    // The number of pages to display around the current page
    displayCount: number = 4,
): (number | typeof SEPARATOR)[] {
    const pages: (number | typeof SEPARATOR)[] = [];

    if (lastPage >= 1) {
        pages.push(1);
    }

    let startPage = Math.max(2, currentPage - Math.floor(displayCount / 2));
    let endPage = Math.min(lastPage, currentPage + Math.floor(displayCount / 2));

    if (endPage - startPage < displayCount) {
        if (currentPage <= displayCount) {
            endPage = Math.min(lastPage, displayCount);
        } else if (currentPage > lastPage - displayCount) {
            startPage = Math.max(2, lastPage - displayCount);
        }
    }

    if (startPage > 2) {
        pages.push(SEPARATOR);
    }

    for (let i = startPage; i <= endPage; i++) {
        if (i !== 1 && i !== lastPage) {
            pages.push(i);
        }
    }

    if (endPage < lastPage - 1) {
        pages.push(SEPARATOR);
    }

    if (lastPage > 1) {
        pages.push(lastPage);
    }

    return pages;
}
