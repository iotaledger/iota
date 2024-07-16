// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgGlobe(props: SVGProps<SVGSVGElement>) {
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
                d="M22 12c0-5.523-4.477-10-10-10S2 6.477 2 12s4.477 10 10 10 10-4.477 10-10Zm-8.253 6.177C12.948 19.774 12.215 20 12 20c-.215 0-.948-.226-1.747-1.823-.639-1.278-1.11-3.083-1.226-5.177h5.946c-.116 2.094-.587 3.899-1.226 5.177ZM14.973 11H9.027c.116-2.094.587-3.899 1.226-5.177C11.052 4.226 11.785 4 12 4c.215 0 .948.226 1.747 1.823.639 1.278 1.11 3.083 1.226 5.177Zm2.002 2c-.12 2.434-.678 4.61-1.512 6.214A8.007 8.007 0 0 0 19.938 13h-2.963Zm2.963-2h-2.963c-.12-2.434-.678-4.61-1.512-6.214A8.007 8.007 0 0 1 19.938 11ZM7.025 11c.12-2.434.678-4.61 1.512-6.214A8.007 8.007 0 0 0 4.062 11h2.963Zm-2.963 2a8.007 8.007 0 0 0 4.475 6.214C7.703 17.61 7.145 15.434 7.025 13H4.062Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
