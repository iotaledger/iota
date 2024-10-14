// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCopyToClipboard } from '@iota/core';
import { Checkmark, Copy } from '@iota/ui-icons';
import { cx } from 'class-variance-authority';
import { motion } from 'framer-motion';
import { useEffect, useState } from 'react';
import { toast } from 'react-hot-toast';
import { Link } from '~/components/ui';

export interface CopyToClipboardProps {
    copyText: string;
    onSuccessMessage?: string;
    color?: string;
}

const TIMEOUT_TIMER = 2000;

export function CopyToClipboard({
    copyText,
    color,
    onSuccessMessage = 'Copied!',
}: CopyToClipboardProps): JSX.Element {
    const [copied, setCopied] = useState(false);
    const copyToClipBoard = useCopyToClipboard(() => toast.success(onSuccessMessage));

    const handleCopy = async () => {
        await copyToClipBoard(copyText);
        setCopied(true);
    };

    useEffect(() => {
        if (copied) {
            const timeout = setTimeout(() => {
                setCopied(false);
            }, TIMEOUT_TIMER);

            return () => clearTimeout(timeout);
        }
    }, [copied]);

    return (
        <Link disabled={copied} onClick={handleCopy}>
            <span className="sr-only">Copy</span>
            {copied ? (
                <Checkmark className={cx({ color })} />
            ) : (
                <motion.div
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ duration: 0.2 }}
                >
                    <Copy className={cx({ color })} />
                </motion.div>
            )}
        </Link>
    );
}
