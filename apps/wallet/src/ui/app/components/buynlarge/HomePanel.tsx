// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Text } from '../../shared/text';
import Close from './close.svg';
import { useBuyNLargeAssets } from './useBuyNLargeAssets';

const SEEN_KEY = 'buy-n-large-seen-v2';

export function BuyNLargeHomePanel() {
    const navigate = useNavigate();
    const [seen, setSeen] = useState<string[]>(() => {
        const stored = localStorage.getItem(SEEN_KEY);
        if (stored) {
            return JSON.parse(stored);
        }
        return [];
    });

    const bnl = useBuyNLargeAssets();

    return (
        <>
            {bnl.map((item, index) => {
                if (!item || !item.enabled || !item.asset || seen.includes(item?.objectType))
                    return null;

                return (
                    <div key={index}>
                        <div
                            role="button"
                            onClick={() => {
                                navigate(
                                    `/nft-details?${new URLSearchParams({
                                        objectId: item.asset?.data?.objectId ?? '',
                                    }).toString()}`,
                                );

                                ampli.clickedCollectibleCard({
                                    objectId: item.asset?.data?.objectId ?? '',
                                    collectibleType: item.asset?.data?.type ?? '',
                                    sourceScreen: 'HomePanel',
                                });
                            }}
                            className="flex w-full flex-row items-center gap-4 rounded-xl px-4 py-3"
                            style={{
                                backgroundColor: item.backgroundColor,
                            }}
                        >
                            <div className="h-8 w-8">
                                <img
                                    src={item.homeImage}
                                    alt=""
                                    className="h-full w-full object-contain"
                                />
                            </div>

                            <div className="flex-1">
                                <Text variant="body" weight="medium" color="white">
                                    {item.homeDescription}
                                </Text>
                            </div>

                            <div>
                                <button
                                    type="button"
                                    aria-label="Close"
                                    className="m-0 border-none bg-transparent p-0"
                                    onClick={(e) => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        const nextSeen = [...new Set([...seen, item?.objectType])];
                                        localStorage.setItem(SEEN_KEY, JSON.stringify(nextSeen));
                                        setSeen(nextSeen);
                                    }}
                                >
                                    <Close
                                        className="text-content-onColor"
                                        width={16}
                                        height={16}
                                    />
                                </button>
                            </div>
                        </div>
                    </div>
                );
            })}
        </>
    );
}
