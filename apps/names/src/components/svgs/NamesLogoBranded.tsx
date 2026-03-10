// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';

export default function SvgNamesLogoBranded(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 15 24"
            {...props}
        >
            <path
                d="M14.2042 8.5203C14.4832 8.5203 14.7095 8.74684 14.7095 9.02628V15.2711C14.7095 15.405 14.6562 15.5337 14.5613 15.6287L7.10258 23.0985C7.00769 23.1935 6.87912 23.2469 6.74498 23.2469H0.509013C0.229966 23.2469 0.00374308 23.0203 0.00374308 22.7409V16.4961C0.00374723 16.3617 0.0570337 16.233 0.151914 16.138L7.75885 8.5203H14.2042Z"
                fill="url(#paint0_linear_11079_3429)"
            />
            <path
                d="M7.25353 0.75C7.53258 0.75 7.75884 0.976545 7.75885 1.25599V8.51984H0.50527C0.226224 8.51983 0 8.29325 0 8.01381V1.25599C1.07934e-05 0.976551 0.226231 0.750011 0.50527 0.75H7.25353Z"
                fill="url(#paint1_linear_11079_3429)"
            />
            <defs>
                <linearGradient
                    id="paint0_linear_11079_3429"
                    x1="14.7095"
                    y1="0.750001"
                    x2="-5.70052"
                    y2="15.1447"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop stopColor="#A139FF" />
                    <stop offset="0.5" stopColor="#3131FF" />
                    <stop offset="1" stopColor="#14F0D6" />
                </linearGradient>
                <linearGradient
                    id="paint1_linear_11079_3429"
                    x1="14.7095"
                    y1="0.750001"
                    x2="-5.70052"
                    y2="15.1447"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop stopColor="#A139FF" />
                    <stop offset="0.5" stopColor="#3131FF" />
                    <stop offset="1" stopColor="#14F0D6" />
                </linearGradient>
            </defs>
        </svg>
    );
}
