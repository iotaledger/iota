// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const IPFS_START_STRING = 'https://ipfs.io/ipfs/';

export const findIPFSvalue = (url: string): string | undefined => url.match(/^ipfs:\/\/(.*)/)?.[1];

export function transformURL(url: string): string {
    const found = findIPFSvalue(url);
    if (!found) {
        return url;
    }
    return `${IPFS_START_STRING}${found}`;
}

export async function extractMediaFileType(
    displayString: string,
    signal: AbortSignal,
): Promise<string> {
    // First check Content-Type in header:
    const result = await fetch(transformURL(displayString), {
        signal: signal,
    })
        .then((resp) => resp?.headers?.get('Content-Type')?.split('/').reverse()?.[0])
        .catch((err) => console.error(err));

    // Return the Content-Type if found:
    if (result) {
        return result;
    }
    // When Content-Type cannot be accessed (e.g. because of CORS), rely on file extension
    const extension = displayString?.split('.').reverse()?.[0] || '';
    if (['jpg', 'jpeg', 'png'].includes(extension)) {
        return extension;
    } else {
        return 'Image';
    }
}
