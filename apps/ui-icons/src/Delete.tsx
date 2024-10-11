// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgDelete(props: SVGProps<SVGSVGElement>) {
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
                d="M8 4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2h4a1 1 0 1 1 0 2h-.069l-.8 11.214A3 3 0 0 1 16.137 22H7.862a3 3 0 0 1-2.992-2.786L4.069 8H4a1 1 0 0 1 0-2h4zM6.074 8l.79 11.071a1 1 0 0 0 .998.929h8.276a1 1 0 0 0 .997-.929L17.926 8zM14 6h-4V4h4zm-4 4a1 1 0 0 1 1 1v6a1 1 0 1 1-2 0v-6a1 1 0 0 1 1-1m4 0a1 1 0 0 1 1 1v6a1 1 0 1 1-2 0v-6a1 1 0 0 1 1-1"
                clipRule="evenodd"
            />
        </svg>
    );
}
