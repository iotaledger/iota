// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgLightMode(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path fill="currentColor" d="M12 2a1 1 0 0 1 1 1v2a1 1 0 1 1-2 0V3a1 1 0 0 1 1-1" />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M7 12a5 5 0 1 1 10 0 5 5 0 0 1-10 0m5-3a3 3 0 1 0 0 6 3 3 0 0 0 0-6"
                clipRule="evenodd"
            />
            <path
                fill="currentColor"
                d="M13 19a1 1 0 1 0-2 0v2a1 1 0 1 0 2 0zm9-7a1 1 0 0 1-1 1h-2a1 1 0 1 1 0-2h2a1 1 0 0 1 1 1M5 13a1 1 0 1 0 0-2H3a1 1 0 1 0 0 2zm14.07-8.071a1 1 0 0 1 0 1.414L17.915 7.5A1 1 0 1 1 16.5 6.086l1.157-1.157a1 1 0 0 1 1.414 0ZM7.5 17.914a1 1 0 1 0-1.414-1.415l-1.157 1.158a1 1 0 1 0 1.414 1.414zm11.57 1.157a1 1 0 0 1-1.413 0L16.5 17.914a1 1 0 0 1 1.414-1.414l1.157 1.157a1 1 0 0 1 0 1.414ZM6.086 7.5A1 1 0 1 0 7.5 6.087L6.343 4.93a1 1 0 1 0-1.414 1.414l1.157 1.158Z"
            />
        </svg>
    );
}
