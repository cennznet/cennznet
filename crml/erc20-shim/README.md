## Token Address Derivation
This module provides a read-only erc20 compatibility shim for generic assets.
The process to construct a 20 byte Ethereum addresses for a generic asset is as follows:
`<modulePrefix: 1 byte><assetId: 4 bytes><padding: 15 bytes>`
generic asset will use the module prefix `1`
prefix allows other modules to derive contract addresses without conflict.

```bash
module prefix: 01
asset id: 00003e81
padding: 000000000000000000000000000000
```
the address for generic assetId `16001` (test CPAY) is: `0x0100003e81000000000000000000000000000000`