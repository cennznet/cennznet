## TODO:
- [ ] Implement challenge protocol
    - [ ] notarize eth_calls
    - [ ] bonding for relayers/challengers
    - [ ] rewards and slashing for relayers/challengers
- [x] Expiry time for callbacks
- [x] Add multi-currency fee payment for the callback
- [x] Improve accurate weights and limit # of callbacks processed by on_idle
- [x] erc20-peg withdraw precompile
- [x] cennzx swap precompile

Other:
- error codes for request failure in callback
- unused gas back to caller
- optimization: appears to be reaping GA account/dust every callback creates