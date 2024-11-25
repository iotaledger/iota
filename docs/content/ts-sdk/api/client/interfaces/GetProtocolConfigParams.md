# Interface: GetProtocolConfigParams

Return the protocol config table for the given version number. If the version number is not
specified, If none is specified, the node uses the version of the latest epoch it has processed.

## Properties

### version?

> `optional` **version**: `null` \| `string`

An optional protocol version specifier. If omitted, the latest protocol config table for the node
will be returned.
