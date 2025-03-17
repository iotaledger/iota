// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading } from '_components';
import { useAppDispatch, useAppSelector, useActiveAddress } from '_hooks';
import { createDappStatusSelector } from '_redux/slices/permissions';
import { ampli } from '_src/shared/analytics/ampli';
import { useClick, useDismiss, useFloating, useInteractions } from '@floating-ui/react';
import { AnimatePresence, motion } from 'framer-motion';
import { memo, useCallback, useMemo, useState } from 'react';
import { ButtonConnectedTo } from '../ButtonConnectedTo';
import { appDisconnect } from './actions';
import { Link } from '@iota/apps-ui-icons';
import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';

function DappStatusComponent() {
    const dispatch = useAppDispatch();
    const activeOriginUrl = useAppSelector(({ app }) => app.activeOrigin);
    const activeOrigin = useMemo(() => {
        try {
            return (activeOriginUrl && new URL(activeOriginUrl).hostname) || null;
        } catch (e) {
            return null;
        }
    }, [activeOriginUrl]);
    const activeOriginFavIcon = useAppSelector(({ app }) => app.activeOriginFavIcon);
    const activeAddress = useActiveAddress();
    const dappStatusSelector = useMemo(
        () => createDappStatusSelector(activeOriginUrl, activeAddress),
        [activeOriginUrl, activeAddress],
    );
    const isConnected = useAppSelector(dappStatusSelector);
    const [disconnecting, setDisconnecting] = useState(false);
    const [visible, setVisible] = useState(false);
    const onHandleClick = useCallback(
        (e: boolean) => {
            if (!disconnecting) {
                setVisible((isVisible) => !isVisible);
            }
        },
        [disconnecting],
    );
    const { x, y, context, reference, refs } = useFloating({
        open: visible,
        onOpenChange: onHandleClick,
        placement: 'bottom',
    });
    const { getFloatingProps, getReferenceProps } = useInteractions([
        useClick(context),
        useDismiss(context, {
            outsidePressEvent: 'click',
            bubbles: false,
        }),
    ]);
    const handleDisconnect = useCallback(async () => {
        if (!disconnecting && isConnected && activeOriginUrl && activeAddress) {
            setDisconnecting(true);
            try {
                await dispatch(
                    appDisconnect({
                        origin: activeOriginUrl,
                        accounts: [activeAddress],
                    }),
                ).unwrap();
                ampli.disconnectedApplication({
                    applicationUrl: activeOriginUrl,
                    disconnectedAccounts: 1,
                    sourceFlow: 'Header',
                });
                setVisible(false);
            } catch (e) {
                // Do nothing
            } finally {
                setDisconnecting(false);
            }
        }
    }, [disconnecting, isConnected, activeOriginUrl, activeAddress, dispatch]);
    if (!isConnected) {
        return null;
    }
    return (
        <>
            <ButtonConnectedTo
                truncate
                iconBefore={<Link className="h-5 w-5" />}
                text={activeOrigin || ''}
                ref={reference}
                {...getReferenceProps()}
            />
            <AnimatePresence>
                {visible ? (
                    <motion.div
                        initial={{
                            opacity: 0,
                            scale: 0,
                            y: 'calc(-50% - 15px)',
                        }}
                        animate={{ opacity: 1, scale: 1, y: 0 }}
                        exit={{ opacity: 0, scale: 0, y: 'calc(-50% - 15px)' }}
                        transition={{
                            duration: 0.3,
                            ease: 'anticipate',
                        }}
                        className="absolute right-6 top-[48px] z-50 max-w-72 rounded-2xl bg-neutral-96 p-sm shadow-xl dark:bg-neutral-12"
                        style={{ top: y || 0, left: x || 0 }}
                        {...getFloatingProps()}
                        ref={refs.setFloating}
                    >
                        <div className="flex flex-col items-center gap-xs">
                            <div className="flex w-full flex-row items-start gap-xs">
                                {activeOriginFavIcon ? (
                                    <div className="h-7 w-7 shrink-0 rounded-full border border-shader-neutral-light-8 p-xxs">
                                        <img
                                            src={activeOriginFavIcon}
                                            alt="App Icon"
                                            className="h-full w-full object-contain"
                                        />
                                    </div>
                                ) : null}
                                <div>
                                    <span className="text-label-md text-neutral-40 dark:text-neutral-60">
                                        Connected to
                                    </span>
                                    <div className="break-all text-body-sm text-neutral-10 dark:text-neutral-92">
                                        {activeOrigin}
                                    </div>
                                </div>
                            </div>
                            <Loading loading={disconnecting}>
                                <div className="self-center">
                                    <Button
                                        onClick={handleDisconnect}
                                        disabled={disconnecting}
                                        size={ButtonSize.Small}
                                        text="Disconnect App"
                                        type={ButtonType.Destructive}
                                    />
                                </div>
                            </Loading>
                        </div>
                    </motion.div>
                ) : null}
            </AnimatePresence>
        </>
    );
}

export const DappStatus = memo(DappStatusComponent);
