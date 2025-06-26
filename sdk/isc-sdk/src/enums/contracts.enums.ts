export enum CoreContract {
    Root = 'root',
    Accounts = 'accounts',
    Blocklog = 'blocklog',
    Governance = 'governance',
    Errors = 'errors',
    evm = 'evm',
}

export enum RootContractMethod {
    FindContract = 'findContract',
    GetContractRecords = 'getContractRecords',
}

export enum AccountsContractMethod {
    Deposit = 'deposit',
    Withdraw = 'withdraw',
    TransferAllowanceTo = 'transferAllowanceTo',
    TransferAccountToChain = 'transferAccountToChain',
    SetCoinMetadata = 'setCoinMetadata',
    DeleteCoinMetadata = 'deleteCoinMetadata',
    Balance = 'balance',
    BalanceBaseToken = 'balanceBaseToken',
    BalanceBaseTokenEVM = 'balanceBaseTokenEVM',
    BalanceCoin = 'balanceCoin',
    TotalAssets = 'totalAssets',
    AccountObjects = 'accountObjects',
    AccountObjectsInCollection = 'accountObjectsInCollection',
    GetAccountNonce = 'getAccountNonce',
    ObjectBCS = 'objectBCS',
}

export enum BlocklogContractMethod {
    GetBlockInfo = 'getBlockInfo',
    GetRequestIDsForBlock = 'getRequestIDsForBlock',
    GetRequestReceipt = 'getRequestReceipt',
    GetRequestReceiptsForBlock = 'getRequestReceiptsForBlock',
    IsRequestProcessed = 'isRequestProcessed',
    GetEventsForRequest = 'getEventsForRequest',
    GetEventsForBlock = 'getEventsForBlock',
}

export enum GovernanceContractMethod {
    RotateStateController = 'rotateStateController',
    AddAllowedStateControllerAddress = 'addAllowedStateControllerAddress',
    RemoveAllowedStateControllerAddress = 'removeAllowedStateControllerAddress',
    DelegateChainOwnership = 'delegateChainOwnership',
    ClaimChainOwnership = 'claimChainOwnership',
    SetFeePolicy = 'setFeePolicy',
    SetGasLimits = 'setGasLimits',
    SetEVMGasRatio = 'setEVMGasRatio',
    AddCandidateNode = 'addCandidateNode',
    RevokeAccessNode = 'revokeAccessNode',
    ChangeAccessNodes = 'changeAccessNodes',
    StartMaintenance = 'startMaintenance',
    StopMaintenance = 'stopMaintenance',
    SetPayoutAgentID = 'setPayoutAgentID',
    GetAllowedStateControllerAddresses = 'getAllowedStateControllerAddresses',
    GetChainOwner = 'getChainOwner',
    GetChainInfo = 'getChainInfo',
    GetFeePolicy = 'getFeePolicy',
    GetGasLimits = 'getGasLimits',
    GetEVMGasRatio = 'getEVMGasRatio',
    GetChainNodes = 'getChainNodes',
    GetMaintenanceStatus = 'getMaintenanceStatus',
    GetPayoutAgentID = 'getPayoutAgentID',
    GetMetadata = 'getMetadata',
}

export enum ErrorsContractMethod {
    RegisterError = 'registerError',
    GetErrorMessageFormat = 'getErrorMessageFormat',
}

export enum EVMContractMethod {
    RegisterERC20Coin = 'registerERC20Coin',
    SendTransaction = 'sendTransaction',
    CallContract = 'callContract',
    RegisterERC721NFTCollection = 'registerERC721NFTCollection',
    NewL1Deposit = 'newL1Deposit',
    GetChainID = 'getChainID',
}
