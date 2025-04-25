// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'clsx';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { useContext, useEffect, useRef, useState } from 'react';

import { NetworkContext } from '~/contexts';
import { CustomRPCInput } from '~/components/ui';
import { ampli } from '~/lib/utils';
import { type NetworkId, getAllNetworks } from '@iota/iota-sdk/client';
import { Button, ButtonSize, ButtonType, Dropdown, ListItem } from '@iota/apps-ui-kit';
import { ArrowDown, CheckmarkFilled } from '@iota/apps-ui-icons';
import { Transition } from '@headlessui/react';

interface NetworkOption {
    id: string;
    label: string;
}

export function NetworkSelector(): JSX.Element {
    const elementRef = useRef<HTMLDivElement>(null);
    const [network, setNetwork] = useContext(NetworkContext);
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: binaryVersion } = useIotaClientQuery('getRpcApiVersion');
    const [isDropdownOpen, setIsDropdownOpen] = useState(false);

    const networks = Object.values(getAllNetworks()).map((network) => ({
        id: network.id,
        label: network.name,
    })) as NetworkOption[];

    const handleNetworkSwitch = (networkId: NetworkId) => {
        ampli.switchedNetwork({ toNetwork: networkId });
        setNetwork(networkId);
    };

    const selectedNetwork = networks.find(({ id }) => id === network);
    const isCustomNetwork = !networks.find(({ id }) => id === network);
    const [customOpen, setCustomOpen] = useState(isCustomNetwork);

    // Hide dropdown on click outside
    useEffect(() => {
        const listener = (event: MouseEvent | TouchEvent) => {
            const el = elementRef?.current;

            if (!el || el.contains(event?.target as Node)) {
                return;
            }

            setIsDropdownOpen(false);
            setCustomOpen(false);
        };

        document.addEventListener('click', listener, true);
        document.addEventListener('touchstart', listener, true);

        return () => {
            document.removeEventListener('click', listener, true);
            document.removeEventListener('touchstart', listener, true);
        };
    }, [elementRef]);

    return (
        <div ref={elementRef} className="relative self-center">
            <Button
                type={ButtonType.Outlined}
                size={ButtonSize.Small}
                text={selectedNetwork?.label ?? 'Custom'}
                icon={
                    <ArrowDown
                        className={cx('-mr-xs transition-all duration-200 ease-in', {
                            'rotate-180': isDropdownOpen,
                        })}
                    />
                }
                iconAfterText
                onClick={() => {
                    setIsDropdownOpen(!isDropdownOpen);
                    if (!isDropdownOpen) {
                        setCustomOpen(false);
                    }
                }}
            />
            <Transition
                show={isDropdownOpen}
                enter="transition duration-300"
                enterFrom="opacity-0 scale-75"
                enterTo="opacity-100 scale-100"
                leave="transition duration-150"
                leaveFrom="opacity-100 scale-100"
                leaveTo="opacity-0 scale-75"
            >
                <div className="absolute right-0 mt-xs w-52">
                    <Dropdown>
                        {networks.map((network, idx) => (
                            <ListItem
                                key={idx}
                                onClick={() => handleNetworkSwitch(network.id)}
                                hideBottomBorder
                                isHighlighted={network === selectedNetwork}
                            >
                                <div className="flex items-center gap-2">
                                    <CheckmarkFilled
                                        className={cx('flex-shrink-0', {
                                            'text-primary-30': network === selectedNetwork,
                                            'text-gray-45': network !== selectedNetwork,
                                        })}
                                    />
                                    {network.label}
                                </div>
                            </ListItem>
                        ))}
                        <ListItem
                            key="custom-rpc"
                            onClick={() => setCustomOpen(true)}
                            hideBottomBorder
                        >
                            <div className="flex flex-col">
                                <div className="flex items-center gap-2">
                                    <CheckmarkFilled
                                        className={cx('flex-shrink-0', {
                                            'text-success': isCustomNetwork,
                                            'text-gray-45': !isCustomNetwork,
                                        })}
                                    />
                                    Custom RPC URL
                                </div>
                                {customOpen && (
                                    <div className="mt-3">
                                        <CustomRPCInput
                                            value={isCustomNetwork ? network : ''}
                                            onChange={handleNetworkSwitch}
                                        />
                                    </div>
                                )}
                            </div>
                        </ListItem>
                        {!!network && data?.protocolVersion && binaryVersion ? (
                            <div className="bg-hero-darkest/5 rounded-b-lg">
                                <div className="flex flex-col justify-between gap-1 px-4 py-3">
                                    <div className="text-body-sm font-medium text-neutral-40">
                                        IOTA {selectedNetwork?.label ?? 'Custom RPC'}
                                    </div>
                                    <div className="text-body-sm font-medium text-neutral-40">
                                        v{binaryVersion} (Protocol {data?.protocolVersion})
                                    </div>
                                </div>
                            </div>
                        ) : null}
                    </Dropdown>
                </div>
            </Transition>
        </div>
    );
}
