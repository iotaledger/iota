// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgQrCode(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M5 3a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zm4 2H5v4h4zm-4 8a2 2 0 0 0-2 2v4a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-4a2 2 0 0 0-2-2zm4 2H5v4h4zm4-10a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2h-4a2 2 0 0 1-2-2zm2 0h4v4h-4z"
                clipRule="evenodd"
            />
            <path
                fill="currentColor"
                d="M21 14a1 1 0 1 1-2 0 1 1 0 0 1 2 0m-7 7a1 1 0 1 0 0-2 1 1 0 0 0 0 2m-1-6a2 2 0 0 1 2-2h1a1 1 0 1 1 0 2h-1v1a1 1 0 1 1-2 0zm6 6a2 2 0 0 0 2-2v-1a1 1 0 1 0-2 0v1h-1a1 1 0 1 0 0 2z"
            />
        </svg>
    );
}
