// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createMessage } from '_messages';
import type { Message } from '_messages';
import type { PortChannelName } from '_src/shared/messaging/portChannelName';
import { isBasePayload, type ErrorPayload } from '_payloads';
import type { LoadedFeaturesPayload } from '_payloads/feature-gating';
import { isSetNetworkPayload, type SetNetworkPayload } from '_payloads/network';
import { isGetPermissionRequests, isPermissionResponse } from '_payloads/permissions';
import type { Permission, PermissionRequests } from '_payloads/permissions';
import { isDisconnectApp } from '_src/shared/messaging/messages/payloads/permissions/disconnectApp';
import type { UpdateActiveOrigin } from '_payloads/tabs/updateActiveOrigin';
import type { ApprovalRequest } from '_src/shared/messaging/messages/payloads/transactions/approvalRequest';
import { isGetTransactionRequests } from '_src/shared/messaging/messages/payloads/transactions/ui/getTransactionRequests';
import type { GetTransactionRequestsResponse } from '_src/shared/messaging/messages/payloads/transactions/ui/getTransactionRequestsResponse';
import { isTransactionRequestResponse } from '_src/shared/messaging/messages/payloads/transactions/ui/transactionRequestResponse';
import Permissions from '_src/background/permissions';
import Tabs from '_src/background/tabs';
import Transactions from '_src/background/transactions';
import { growthbook } from '_src/shared/experimentation/features';
import {
    isMethodPayload,
    type MethodPayload,
    type UIAccessibleEntityType,
} from '_src/shared/messaging/messages/payloads/methodPayload';
import { toEntropy } from '_src/shared/utils';
import Dexie from 'dexie';
import { BehaviorSubject, filter, switchMap, takeUntil } from 'rxjs';
import Browser from 'webextension-polyfill';
import type { Runtime } from 'webextension-polyfill';

import {
    accountSourcesHandleUIMessage,
    getAccountSourceByID,
    getAllSerializedUIAccountSources,
} from '../account-sources';
import { accountSourcesEvents } from '../account-sources/events';
import { MnemonicAccountSource } from '../account-sources/mnemonicAccountSource';
import {
    accountsHandleUIMessage,
    addNewAccounts,
    getAccountsByAddress,
    getAllSerializedUIAccounts,
} from '../accounts';
import { accountsEvents } from '../accounts/events';
import { getAutoLockMinutes, notifyUserActive, setAutoLockMinutes } from '../autoLockAccounts';
import { backupDB, getDB, SETTINGS_KEYS } from '../db';
import { clearStatus, doMigration, getStatus } from '../storageMigration';
import NetworkEnv from '../networkEnv';
import { Connection } from './connection';
import { SeedAccountSource } from '../account-sources/seedAccountSource';
import { AccountSourceType } from '../account-sources/accountSource';
import { isDeriveBipPathAccountsFinder, isPersistAccountsFinder } from '_payloads/accounts-finder';
import type { SerializedAccount } from '../accounts/account';
import { LedgerAccount } from '../accounts/ledgerAccount';

export class UiConnection extends Connection {
    public static readonly CHANNEL: PortChannelName = 'iota_ui<->background';
    private uiAppInitialized: BehaviorSubject<boolean> = new BehaviorSubject(false);

    constructor(port: Runtime.Port) {
        super(port);
        this.uiAppInitialized
            .pipe(
                filter((init) => init),
                switchMap(() => Tabs.activeOrigin),
                takeUntil(this.onDisconnect),
            )
            .subscribe(({ origin, favIcon }) => {
                this.send(
                    createMessage<UpdateActiveOrigin>({
                        type: 'update-active-origin',
                        origin,
                        favIcon,
                    }),
                );
            });
    }

    public async notifyEntitiesUpdated(entitiesType: UIAccessibleEntityType) {
        this.send(
            createMessage<MethodPayload<'entitiesUpdated'>>({
                type: 'method-payload',
                method: 'entitiesUpdated',
                args: {
                    type: entitiesType,
                },
            }),
        );
    }

    protected async handleMessage(msg: Message) {
        const { payload, id } = msg;
        try {
            if (isGetPermissionRequests(payload)) {
                this.sendPermissions(Object.values(await Permissions.getPermissions()), id);
                // TODO: we should depend on a better message to know if app is initialized
                if (!this.uiAppInitialized.value) {
                    this.uiAppInitialized.next(true);
                }
            } else if (isPermissionResponse(payload)) {
                Permissions.handlePermissionResponse(payload);
            } else if (isTransactionRequestResponse(payload)) {
                Transactions.handleMessage(payload);
            } else if (isGetTransactionRequests(payload)) {
                this.sendTransactionRequests(
                    Object.values(await Transactions.getTransactionRequests()),
                    id,
                );
            } else if (isDisconnectApp(payload)) {
                await Permissions.delete(payload.origin, payload.specificAccounts);
                this.send(createMessage({ type: 'done' }, id));
            } else if (isBasePayload(payload) && payload.type === 'get-features') {
                await growthbook.loadFeatures();
                this.send(
                    createMessage<LoadedFeaturesPayload>(
                        {
                            type: 'features-response',
                            features: growthbook.getFeatures(),
                            attributes: growthbook.getAttributes(),
                        },
                        id,
                    ),
                );
            } else if (isBasePayload(payload) && payload.type === 'get-network') {
                this.send(
                    createMessage<SetNetworkPayload>(
                        {
                            type: 'set-network',
                            network: await NetworkEnv.getActiveNetwork(),
                        },
                        id,
                    ),
                );
            } else if (isSetNetworkPayload(payload)) {
                await NetworkEnv.setActiveNetwork(payload.network);
                this.send(createMessage({ type: 'done' }, id));
            } else if (isMethodPayload(payload, 'getStoredEntities')) {
                const entities = await this.getUISerializedEntities(payload.args.type);
                this.send(
                    createMessage<MethodPayload<'storedEntitiesResponse'>>(
                        {
                            method: 'storedEntitiesResponse',
                            type: 'method-payload',
                            args: {
                                type: payload.args.type,
                                entities,
                            },
                        },
                        msg.id,
                    ),
                );
            } else if (await accountSourcesHandleUIMessage(msg, this)) {
                return;
            } else if (await accountsHandleUIMessage(msg, this)) {
                return;
            } else if (isMethodPayload(payload, 'getStorageMigrationStatus')) {
                this.send(
                    createMessage<MethodPayload<'storageMigrationStatus'>>(
                        {
                            method: 'storageMigrationStatus',
                            type: 'method-payload',
                            args: {
                                status: await getStatus(),
                            },
                        },
                        id,
                    ),
                );
            } else if (isMethodPayload(payload, 'doStorageMigration')) {
                await doMigration(payload.args.password);
                this.send(createMessage({ type: 'done' }, id));
            } else if (isMethodPayload(payload, 'clearWallet')) {
                await Browser.storage.local.clear();
                await Browser.storage.local.set({
                    v: -1,
                });
                clearStatus();
                const db = await getDB();
                await db.delete();
                await db.open();
                // prevents future run of auto backup process of the db (we removed everything nothing to backup after logout)
                await db.settings.put({ setting: SETTINGS_KEYS.isPopulated, value: true });
                this.send(createMessage({ type: 'done' }, id));
            } else if (isMethodPayload(payload, 'getAutoLockMinutes')) {
                this.send(
                    createMessage<MethodPayload<'getAutoLockMinutesResponse'>>(
                        {
                            type: 'method-payload',
                            method: 'getAutoLockMinutesResponse',
                            args: { minutes: await getAutoLockMinutes() },
                        },
                        msg.id,
                    ),
                );
            } else if (isMethodPayload(payload, 'setAutoLockMinutes')) {
                await setAutoLockMinutes(payload.args.minutes);
                this.send(createMessage({ type: 'done' }, msg.id));
                return true;
            } else if (isMethodPayload(payload, 'notifyUserActive')) {
                notifyUserActive();
                this.send(createMessage({ type: 'done' }, msg.id));
                return true;
            } else if (isMethodPayload(payload, 'resetPassword')) {
                const { password, recoveryData } = payload.args;
                if (!recoveryData.length) {
                    throw new Error('Missing recovery data');
                }
                for (const data of recoveryData) {
                    const { accountSourceID, type } = data;
                    const accountSource = await getAccountSourceByID(accountSourceID);
                    if (!accountSource) {
                        throw new Error('Account source not found');
                    }
                    if (
                        !(accountSource instanceof MnemonicAccountSource) &&
                        !(accountSource instanceof SeedAccountSource)
                    ) {
                        throw new Error('Invalid account source type');
                    }
                    if (type === AccountSourceType.Mnemonic) {
                        await accountSource.verifyRecoveryData(data.entropy);
                    }
                    if (type === AccountSourceType.Seed) {
                        await accountSource.verifyRecoveryData(data.seed);
                    }
                }
                const db = await getDB();
                const accountSourceIDs = recoveryData.map(({ accountSourceID }) => accountSourceID);
                await db.transaction('rw', db.accountSources, db.accounts, async () => {
                    await db.accountSources.where('id').noneOf(accountSourceIDs).delete();
                    await db.accounts
                        .filter(
                            (anAccount) =>
                                !('sourceID' in anAccount) ||
                                typeof anAccount.sourceID !== 'string' ||
                                !accountSourceIDs.includes(anAccount.sourceID),
                        )
                        .delete();
                    for (const data of recoveryData) {
                        const { accountSourceID, type } = data;
                        if (type === AccountSourceType.Mnemonic) {
                            await db.accountSources.update(accountSourceID, {
                                encryptedData: await Dexie.waitFor(
                                    MnemonicAccountSource.createEncryptedData(
                                        toEntropy(data.entropy),
                                        password,
                                    ),
                                ),
                            });
                        }
                        if (type === AccountSourceType.Seed) {
                            await db.accountSources.update(accountSourceID, {
                                encryptedData: await Dexie.waitFor(
                                    SeedAccountSource.createEncryptedData(data.seed, password),
                                ),
                            });
                        }
                    }
                });
                await backupDB();
                accountSourcesEvents.emit('accountSourcesChanged');
                accountsEvents.emit('accountsChanged');
                this.send(createMessage({ type: 'done' }, msg.id));
            } else if (isDeriveBipPathAccountsFinder(payload)) {
                const accountSource = await getAccountSourceByID(payload.sourceID);

                if (!accountSource) {
                    throw new Error('Could not find account source');
                }

                const publicKey = await accountSource.derivePubKey(payload.derivationOptions);

                this.send(
                    createMessage(
                        {
                            type: 'derive-bip-path-accounts-finder-response',
                            publicKey: publicKey.toBase64(),
                        },
                        msg.id,
                    ),
                );
            } else if (isPersistAccountsFinder(payload)) {
                let derivedAccounts: Omit<SerializedAccount, 'id'>[] = [];

                switch (payload.sourceStrategy.type) {
                    case 'software':
                        const accountSource = await getAccountSourceByID(
                            payload.sourceStrategy.sourceID,
                        );

                        if (!accountSource) {
                            throw new Error('Could not find account source');
                        }

                        derivedAccounts = await Promise.all(
                            payload.sourceStrategy.bipPaths.map((addressBipPath) =>
                                accountSource.deriveAccount(addressBipPath),
                            ),
                        );
                        break;
                    case 'ledger':
                        for (const address of payload.sourceStrategy.addresses) {
                            derivedAccounts.push(
                                await LedgerAccount.createNew({
                                    password: payload.sourceStrategy.password,
                                    ...address,
                                }),
                            );
                        }
                }

                // Filter those accounts that already exist so they are not duplicated
                const derivedAccountsNonExistent: Omit<SerializedAccount, 'id'>[] = (
                    await Promise.all(
                        derivedAccounts.map(async (account) => {
                            const foundAccounts = await getAccountsByAddress(account.address);
                            for (const foundAccount of foundAccounts) {
                                if (foundAccount.type === account.type) {
                                    // Do not persist accounts with the same address and type
                                    return undefined;
                                }
                            }

                            return account;
                        }),
                    )
                ).filter(Boolean) as Omit<SerializedAccount, 'id'>[];

                // Actually persist the accounts
                await addNewAccounts(derivedAccountsNonExistent);

                this.send(createMessage({ type: 'done' }, msg.id));
            } else {
                throw new Error(
                    `Unhandled message ${msg.id}. (${JSON.stringify(
                        'error' in payload ? `${payload.code}-${payload.message}` : payload.type,
                    )})`,
                );
            }
        } catch (e) {
            this.send(
                createMessage<ErrorPayload>(
                    {
                        error: true,
                        code: -1,
                        message: (e as Error).message,
                    },
                    id,
                ),
            );
        }
    }

    private sendPermissions(permissions: Permission[], requestID: string) {
        this.send(
            createMessage<PermissionRequests>(
                {
                    type: 'permission-request',
                    permissions,
                },
                requestID,
            ),
        );
    }

    private sendTransactionRequests(txRequests: ApprovalRequest[], requestID: string) {
        this.send(
            createMessage<GetTransactionRequestsResponse>(
                {
                    type: 'get-transaction-requests-response',
                    txRequests,
                },
                requestID,
            ),
        );
    }

    private getUISerializedEntities(type: UIAccessibleEntityType) {
        switch (type) {
            case 'accounts': {
                return getAllSerializedUIAccounts();
            }
            case 'accountSources': {
                return getAllSerializedUIAccountSources();
            }
            default: {
                throw new Error(`Unknown entity type ${type}`);
            }
        }
    }
}
