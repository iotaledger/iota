module.exports = {
    'causal history': 'Causal history is the relationship between an object in IOTA and its direct predecessors and successors. This history is essential to the causal order IOTA uses to process transactions. In contrast, other blockchains read the entire state of their world for each transaction, introducing latency.',
    'causal order': '<a href="https://www.scattered-thoughts.net/writing/causal-ordering/">Causal order</a> is a representation of the relationship between transactions and the objects they produce, laid out as dependencies. Validators cannot execute a transaction dependent on objects created by a prior transaction that has not finished. Rather than total order, IOTA uses causal order (a partial order).',
    certificate: 'A certificate is the mechanism proving a transaction was approved or certified. Validators vote on transactions, and aggregators collect a Byzantine-resistant majority of these votes into a certificate and broadcasts it to all IOTA validators, thereby ensuring finality.',
    epoch: 'Operation of the IOTA network is temporally partitioned into non-overlapping, fixed-duration epochs. During a particular epoch, the set of validators participating in the network is fixed.',
    equivocation: 'Equivocation in blockchains is the malicious action of dishonest actors giving conflicting information for the same message, such as inconsistent or duplicate voting.',
    'eventual consistency': '<a href="https://en.wikipedia.org/wiki/Eventual_consistency">Eventual consistency</a> is the consensus model employed by IOTA; if one honest validator certifies the transaction, all of the other honest validators will too eventually.',
    finality: '<a href="https://medium.com/mechanism-labs/finality-in-blockchain-consensus-d1f83c120a9a">Finality</a> is the assurance a transaction will not be revoked. This stage is considered closure for an exchange or other blockchain transaction.',
    gas: '<a href="https://ethereum.org/en/developers/docs/gas/">Gas</a> refers to the computational effort required for executing operations on the IOTA network. In IOTA, gas is paid with the network\'s native currency IOTA. The cost of executing a transaction in IOTA units is referred to as the transaction fee.',
    genesis: 'Genesis is the initial act of creating accounts and gas objects for an IOTA network. IOTA provides a `genesis` command that allows users to create and inspect the genesis object setting up the network for operation.',
    'multi-writer objects': 'Multi-writer objects are objects that are owned by more than one address. Transactions affecting multi-writer objects require consensus in IOTA. This contrasts with transactions affecting only single-writer objects, which require only a confirmation of the owner\'s address contents.',
    object: `
        The basic unit of storage in IOTA is object. In contrast to many other blockchains, where storage is centered around address and each address contains a key-value store, IOTA\'s storage is centered around objects. IOTA objects have one of the following primary states:<br/>
        <br/>
        - <i>Immutable</i> - the object cannot be modified.<br/>
        - <i>Mutable</i> - the object can be changed.<br/>
        <br/>
        Further, mutable objects are divided into these categories:<br/>
        <br/>
        - <i>Owned</i> - the object can be modified only by its owner.<br/>
        - <i>Shared</i> - the object can be modified by anyone.<br/>
        <br/>
        Immutable objects do not need this distinction because they have no owner.
    `,
    pos: '<a href="https://en.wikipedia.org/wiki/Proof_of_stake">Proof-of-stake</a> is a blockchain consensus mechanism where the voting weights of validators or validators is proportional to a bonded amount of the network\'s native currency (called their stake in the network). This mitigates <a href="https://en.wikipedia.org/wiki/Sybil_attack">Sybil attacks</a> by forcing bad actors to gain a large stake in the blockchain first.',
    'single-writer objects': 'Single-writer objects are owned by one address. In IOTA, transactions affecting only single-writer objects owned by the same address may proceed with only a verification of the sender\'s address, greatly speeding transaction times. These are simple transactions. See Single-Writer Apps for example applications of this simple transaction model.',
    'smart contract': 'A <a href="https://en.wikipedia.org/wiki/Smart_contract">smart contract</a> is an agreement based upon the protocol for conducting transactions in a blockchain. In IOTA, smart contracts are written in <a href="../developer/evm-to-move">Solidity/EVM or Move</a>.',
    iota: 'IOTA refers to the IOTA blockchain, and the <a href="https://github.com/iotaledger/iota/">IOTA open source project</a> as a whole, or the native token to the IOTA network.',
    'total order': '<a href="https://en.wikipedia.org/wiki/Total_order">Total order</a> refers to the ordered presentation of the history of all transactions processed by a traditional blockchain up to a given time. This is maintained by many blockchain systems, as the only way to process transactions. In contrast, IOTA uses a causal (partial) order wherever possible and safe.',
    transaction: `
        A transaction in IOTA is a change to the blockchain. This may be a <i>simple transaction</i> affecting only single-writer, single-address objects, such as minting an NFT or transferring an NFT or another token. These transactions may bypass the consensus protocol in IOTA.<br/>
        More <i>complex transactions</i> affecting objects that are shared or owned by multiple addresses, such as asset management and other DeFi use cases, go through the<a href = "https://github.com/iotaledger/iota/tree/develop/narwhal">Narwhal and Bullshark</a> DAG - based mempool and efficient Byzantine Fault Tolerant(BFT) consensus.
    `,
    transfer: 'A transfer is switching the owner address of a token to a new one via command in IOTA. This is accomplished via the IOTA CLI client command line interface. It is one of the more common of many commands available in the CLI client.',
    dag: 'A Directed Acyclic Graph (DAG) is a data structure where nodes are connected in a one-way, non-cyclic manner. In blockchain, DAGs improve scalability by allowing parallel transaction processing without a single-chain bottleneck.',
    bft: 'A Byzantine Fault Tolerant (BFT) consensus protocol enables a distributed network to reach agreement despite malicious or faulty nodes. It ensures reliability as long as most nodes are honest.',
    validator: 'A validator in IOTA plays a passive role analogous to the more active role of validators and minors in other blockchains. In IOTA, validators do not continuously participate in the consensus protocol but are called into action only when receiving a transaction or certificate.'};
