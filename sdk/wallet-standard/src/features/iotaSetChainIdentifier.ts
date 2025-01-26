import { ChainIdentifier } from '../types';

/**
 * A Wallet Standard feature for setting the chain identifier in a wallet.
 * This feature allows dApps to specify which network/chain the wallet should connect to.
 */
export type IotaSetChainIdentifierFeature = {
    /** Namespace for the feature. */
    'iota:setChainIdentifier': {
        /** Version of the feature API. */
        version: '1.0.0';
        SetChainIdentifier: IotaSetChainIdentifierMethod;
    };
};

/** Method to set the chain identifier in the wallet */
export type IotaSetChainIdentifierMethod = (input: IotaSetChainIdentifierInput) => Promise<void>;

/** Chain identifier to be set in the wallet */
export type IotaSetChainIdentifierInput = ChainIdentifier;
