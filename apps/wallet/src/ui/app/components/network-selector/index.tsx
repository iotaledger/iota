// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppDispatch, useAppSelector } from '_hooks';
import { changeActiveNetwork } from '_redux/slices/app';
import { ampli } from '_src/shared/analytics/ampli';
import { getCustomNetwork } from '_src/shared/api-env';
import { Check24 } from '@iota/icons';
import { getAllNetworks, Network, type NetworkConfiguration } from '@iota/iota-sdk/client';
import cl from 'clsx';
import { AnimatePresence, motion } from 'framer-motion';
import { useEffect, useMemo, useState } from 'react';
import { toast } from 'react-hot-toast';

import { CustomRPCInput } from './custom-rpc-input';
import st from './NetworkSelector.module.scss';

export function NetworkSelector() {
    const activeNetwork = useAppSelector(({ app }) => app.network);
    const activeCustomRpc = useAppSelector(({ app }) => app.customRpc);
    const [isCustomRpcInputVisible, setCustomRpcInputVisible] = useState<boolean>(
        activeNetwork === Network.Custom,
    );
    // change the selected network name whenever the activeNetwork or activeCustomRpc changes
    useEffect(() => {
        setCustomRpcInputVisible(activeNetwork === Network.Custom && !!activeCustomRpc);
    }, [activeNetwork, activeCustomRpc]);
    const dispatch = useAppDispatch();
    const networks = useMemo(() => {
        const supportedNetworks = Object.entries(getAllNetworks());
        const customNetwork: [Network, NetworkConfiguration] = [Network.Custom, getCustomNetwork()];
        return [...supportedNetworks, customNetwork];
    }, []);

    return (
        <div className={st.networkOptions}>
            <ul className={st.networkLists}>
                {networks.map(([id, network]) => (
                    <li className={st.networkItem} key={id}>
                        <button
                            type="button"
                            onClick={async () => {
                                if (activeNetwork === network.id) {
                                    return;
                                }
                                setCustomRpcInputVisible(network.id === Network.Custom);
                                if (network.id !== Network.Custom) {
                                    try {
                                        await dispatch(
                                            changeActiveNetwork({
                                                network: {
                                                    network: network.id,
                                                    customRpcUrl: null,
                                                },
                                                store: true,
                                            }),
                                        ).unwrap();
                                        ampli.switchedNetwork({
                                            toNetwork: network.name,
                                        });
                                    } catch (e) {
                                        toast.error((e as Error).message);
                                    }
                                }
                            }}
                            className={st.networkSelector}
                        >
                            <Check24
                                className={cl(
                                    st.networkIcon,
                                    st.selectedNetwork,
                                    activeNetwork === network.id && st.networkActive,
                                    network.id === Network.Custom &&
                                        isCustomRpcInputVisible &&
                                        st.customRpcActive,
                                )}
                            />

                            {network.name}
                        </button>
                    </li>
                ))}
            </ul>
            <AnimatePresence>
                {isCustomRpcInputVisible && (
                    <motion.div
                        initial={{
                            opacity: 0,
                        }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{
                            duration: 0.5,
                            ease: 'easeInOut',
                        }}
                        className={st.customRpc}
                    >
                        <CustomRPCInput />
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
}

export default NetworkSelector;
