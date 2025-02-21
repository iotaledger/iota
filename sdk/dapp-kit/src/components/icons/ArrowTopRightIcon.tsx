// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ComponentProps } from 'react';

export function ArrowTopRightIcon(props: ComponentProps<'svg'>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            {...props}
        >
            <path
                d="M17.4825 6.29014C17.3873 6.19563 17.2778 6.12421 17.161 6.07588C17.0431 6.02699 16.9138 6 16.7782 6L8.70711 6C8.15482 6 7.70711 6.44772 7.70711 7C7.70711 7.55228 8.15482 8 8.70711 8L14.364 8L6.29289 16.0711C5.90237 16.4616 5.90237 17.0948 6.29289 17.4853C6.68342 17.8758 7.31658 17.8758 7.70711 17.4853L15.7782 9.41421V15.0711C15.7782 15.6234 16.2259 16.0711 16.7782 16.0711C17.3305 16.0711 17.7782 15.6234 17.7782 15.0711L17.7782 7.0008C17.7782 6.86458 17.7509 6.73391 17.7016 6.61556C17.6531 6.49895 17.5816 6.38966 17.487 6.29464L17.4853 6.29289L17.4825 6.29014Z"
                fill="currentColor"
            />
        </svg>
    );
}
