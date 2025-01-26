import { ChainIdentifier } from '../types';

/**
 * A Wallet Standard feature for retrieving the chain identifier from a wallet.
 * This feature allows dApps to determine which network/chain the wallet is connected to.
 */
export type IotaGetChainIdentifierFeature = {
    /** Namespace for the feature. */
    'iota:getChainIdentifier': {
        /** Version of the feature API. */
        version: '1.0.0';
        GetChainIdentifier: IotaGetChainIdentifierMethod;
    };
};

/** Method to retrieve the chain identifier from the wallet */
export type IotaGetChainIdentifierMethod = () => Promise<IotaGetChainIdentifierOutput>;

/** Chain identifier returned by the wallet */
export type IotaGetChainIdentifierOutput = ChainIdentifier;
