// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from 'react';

interface CountdownReturn {
    milliseconds: number;
    stop: () => void;
}

export function useCountdown(initialMilliseconds: number): CountdownReturn {
    const [milliseconds, setMilliseconds] = useState<number>(initialMilliseconds);
    const [isActive, setIsActive] = useState<boolean>(true);
    const intervalRef = useRef<NodeJS.Timeout | null>(null);

    useEffect(() => {
        if (isActive) {
            if (milliseconds > 0) {
                intervalRef.current = setInterval(() => {
                    setMilliseconds((prev) => prev - 1000);
                }, 1000);
            } else if (milliseconds === 0) {
                setIsActive(false);
            }
        }

        return () => {
            if (intervalRef.current) {
                clearInterval(intervalRef.current);
            }
        };
    }, [isActive, milliseconds]);

    const stop = (): void => {
        setIsActive(false);
    };

    return { milliseconds, stop };
}
