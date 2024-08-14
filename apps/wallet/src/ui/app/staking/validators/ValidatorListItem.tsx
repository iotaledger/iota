// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { CheckFill16 } from '@iota/icons';
import { cx } from 'class-variance-authority';
import { AnimatePresence, motion } from 'framer-motion';

import { ValidatorLogo } from './ValidatorLogo';

interface ValidatorListItemProps {
    selected?: boolean;
    value: string | number;
    validatorAddress: string;
}

export function ValidatorListItem({ selected, value, validatorAddress }: ValidatorListItemProps) {
    return (
        <AnimatePresence>
            <motion.div
                whileHover={{ scale: 0.98 }}
                animate={selected ? { scale: 0.98 } : { scale: 1 }}
            >
                <div
                    className={cx(
                        selected ? 'bg-iota/10' : '',
                        'hover:bg-iota/10 group flex w-full items-center justify-between gap-1 rounded-lg px-2 py-3.5',
                    )}
                    role="button"
                >
                    <div className="flex items-center justify-start gap-2.5">
                        <div className="relative flex w-full gap-0.5">
                            {selected && (
                                <CheckFill16
                                    fill="fillCurrent"
                                    className="text-success absolute -translate-y-1 translate-x-4 rounded-full bg-white text-heading6"
                                />
                            )}
                            <ValidatorLogo
                                validatorAddress={validatorAddress}
                                showAddress
                                iconSize="md"
                                size="body"
                                showActiveStatus
                            />
                        </div>
                    </div>
                    <div className="flex items-center gap-0.5">
                        <div className="flex gap-0.5 leading-none">
                            <Text variant="body" weight="semibold" color="steel-darker">
                                {value}
                            </Text>
                            <div
                                className={cx(
                                    selected ? '!opacity-100' : '',
                                    'text-steel flex h-3 items-baseline text-subtitle opacity-0 group-hover:opacity-100',
                                )}
                            ></div>
                        </div>
                    </div>
                </div>
            </motion.div>
        </AnimatePresence>
    );
}
