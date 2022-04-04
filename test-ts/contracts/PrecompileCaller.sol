// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

// Calls GA CENNZ precompile
contract PrecompileCaller {
    // derived CENNZ token address on testnets (AssetId 16000)
    // cccccccc (prefix) + 00003e80 (assetId) + padding
    // run through web3.utils.toChecksumAddress(..)
    address cennz = 0xcCccccCc00003E80000000000000000000000000;

    receive() external payable {}

    function balanceOfProxy(address who) public view returns (uint256) {
        (bool success, bytes memory returnData) = cennz.staticcall(abi.encodeWithSignature("balanceOf(address)", who));
        assembly {
            if eq(success, 0) {
                revert(add(returnData, 0x20), returndatasize())
            }
        }
        return abi.decode(returnData, (uint256));
    }

    // transfer CENNZ from caller using the CENNZ precompile address w ERC20 abi
    function takeCENNZ(uint256 amount) external {
        (bool success, bytes memory returnData) = cennz.call(abi.encodeWithSignature("transferFrom(address,address,uint256)", msg.sender, address(this), amount));
        assembly {
            if eq(success, 0) {
                revert(add(returnData, 0x20), returndatasize())
            }
        }
    }

    // Test sending various CPAY amounts via the EVM.
    // desintation should have 0 balance to start.
    function sendCPAYAmounts(address payable destination) public payable {
        assert(address(destination).balance == 0);
        uint64[8] memory amounts_18 = [1 ether, 1000050000000000000 wei, 1000000000000000001 wei, 1000100000000000000 wei, 1000000000000000000 wei, 999 wei, 1 wei, 0 wei];
	    uint16[8] memory amounts_4 = [10000, 10001, 10001, 10001, 10000, 1, 1, 0];
        uint256 total;

        for(uint i; i < 6; i++) {
            (bool sent, bytes memory _data) = destination.call{value: uint256(amounts_18[i])}("");
            require(sent, "Failed to send CPAY");
            total += (uint256(amounts_4[i]) * uint256(1e14));
            require(total == address(destination).balance, "unexpected balance");
        }
    }
}