// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgVisibilityOff(props: SVGProps<SVGSVGElement>) {
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
                d="M18.72 6.695a1 1 0 1 0-1.44-1.39l-1.518 1.572C14.601 6.348 13.331 6 12 6 8.739 6 5.843 8.092 4.017 9.805c-1.29 1.212-1.29 3.178 0 4.39a18 18 0 0 0 2.666 2.083l-1.695 1.755a1 1 0 0 0 1.438 1.39zM8.67 14.219l5.431-5.623a4 4 0 0 0-5.43 5.623Z"
                clipRule="evenodd"
            />
            <path
                fill="currentColor"
                d="M16 12a4 4 0 0 1-3.975 4l-1.749 1.81c.56.122 1.136.19 1.724.19 3.26 0 6.157-2.092 7.983-3.805 1.29-1.212 1.29-3.178 0-4.39a20 20 0 0 0-1.071-.936l-2.914 3.017z"
            />
        </svg>
    );
}
