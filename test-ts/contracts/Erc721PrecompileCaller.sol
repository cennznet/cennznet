// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.14;

contract Erc721PrecompileCaller {
    function transferFromProxy(
        address precompile,
        address from,
        address to,
        uint256 token_id
    ) external {
        // Calling IERC721(precompile).transferFrom(from, to, token) and IERC721(precompile).approve(to, token)
        // doesn't work using the IERC721 cast. This is because solidity inserts an EXTCODESIZE check when calling a
        // contract with this casting syntax. when it calls a precompile address EXTCODESIZE is 0 so it reverts,
        // doing address.call{} syntax doesn’t insert this check so it works.
        (bool success,) = precompile.call(
            abi.encodeWithSignature(
                "transferFrom(address,address,uint256)",
                from,
                to,
                token_id
            )
        );
        require(success, "call failed");
    }
}
