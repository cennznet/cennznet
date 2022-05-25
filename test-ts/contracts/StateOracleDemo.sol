// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

contract StateOracleDemo {
    // log on state oracle request
    event HiToEthereum(uint256 requestId);
    // log on state oracle response
    event HiFromEthereum(uint256 requestId, uint256 timestamp, uint256 balance);
    event Greeted(bytes32);

    address constant STATE_ORACLE = address(27572);

    receive() external payable {}

    // Make a request for ERC20 balance ('remoteToken') of 'who'
    function helloEthereum(address remoteToken, address who) payable external {
        bytes memory balanceOfCall = abi.encodeWithSignature("balanceOf(address)", who);
        bytes4 callbackSelector = this.ethereumSaysHi.selector;
        uint256 callbackGasLimit = 400_000;
        uint256 callbackBounty = 2 ether; // == 2 cpay

        bytes memory remoteCallRequest = abi.encodeWithSignature("remoteCall(address,bytes,bytes4,uint256,uint256)", remoteToken, balanceOfCall, callbackSelector, callbackGasLimit, callbackBounty);

        (bool success, bytes memory returnData) = STATE_ORACLE.call(remoteCallRequest);
        require(success);

        uint256 requestId = abi.decode(returnData, (uint256));
        emit HiToEthereum(requestId);
    }

    // Receive state oracle response
    function ethereumSaysHi(uint256 requestId, uint256 timestamp, bytes32 returnData) external {
        require(msg.sender == STATE_ORACLE, "must be state oracle");
        uint256 balanceOf = uint256(returnData);

        emit HiFromEthereum(
            requestId,
            timestamp,
            balanceOf
        );
    }
}