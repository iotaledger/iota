// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const MAX_VIDEO_SIZE_MB = 50;
const MAX_VIDEO_SIZE_BYTES = MAX_VIDEO_SIZE_MB * 1024 * 1024;

export function shouldNFTVideoAutoplay(contentLength: string | null | undefined): boolean {
    if (contentLength) {
        const sizeInBytes = parseInt(contentLength, 10);
        return sizeInBytes <= MAX_VIDEO_SIZE_BYTES;
    }
    return false;
}
