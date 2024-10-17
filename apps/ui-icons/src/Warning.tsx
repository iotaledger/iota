// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgWarning(props: SVGProps<SVGSVGElement>) {
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
                d="M13.732 3c-.77-1.333-2.694-1.333-3.464 0l-8.66 15c-.77 1.333.192 3 1.732 3h17.32c1.54 0 2.502-1.667 1.732-3zM12 17.8a1.2 1.2 0 1 0 0-2.4 1.2 1.2 0 0 0 0 2.4m-1.2-9.6a1.2 1.2 0 0 1 2.4 0v3.6a1.2 1.2 0 0 1-2.4 0z"
                clipRule="evenodd"
            />
        </svg>
    );
}
