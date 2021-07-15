// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

import "../interfaces/Ownable.sol";
import "../interfaces/IERC20.sol";
import "../libraries/SafeMath.sol";

contract CENNZnetBridge is Ownable {
    using SafeMath for uint256;

    bool isBridgeActive;
    uint32 nonce;
    mapping (address => mapping(address => uint)) public balances;

    event Deposit(address indexed, address tokenType, uint256 amount, bytes32 cennznetAddress, uint256 timestamp);

    // Deposit amount of tokenType
    // the pegged version of the token will be claim-able on CENNZnet
    function deposit(address tokenType, uint256 amount, bytes32 cennznetAddress) external {
        require(isBridgeActive, "deposits paused");
        require(amount > 0, "no tokens deposited");
        require(IERC20(tokenType).transferFrom(msg.sender, address(this), amount), "deposit failed");
        balances[msg.sender][tokenType] = balances[msg.sender][tokenType].add(amount);

        emit Deposit(msg.sender, tokenType, amount, cennznetAddress, block.timestamp);
    }

    function activateDeposits() external onlyOwner {
        isBridgeActive = true;
    }

    function pauseDeposits() external onlyOwner {
        isBridgeActive = false;
    }
}
