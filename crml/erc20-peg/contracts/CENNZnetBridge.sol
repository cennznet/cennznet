// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";

contract CENNZnetBridge is Ownable {
    using SafeMath for uint256;

    // whether the bridge is accepting deposits
    bool public depositsActive;
    // whether the bridge is accepting withdrawals
    bool public withdrawalsActive;
    // map from integer index to validator ECDSA address
    // this is the CENNZnet validator ECDSA session keys
    address[] public validators;
    // Nonce for validator set changes
    uint32 validatorSetNonce;
    // Global withdrawal nonces
    mapping(uint => bool) public withdrawawlNonce;
    // withdrawal fee, offsets bridge upkeep costs
    uint withdrawalFee = 1e14;

    event Deposit(address indexed, address tokenType, uint256 amount, bytes32 cennznetAddress);
    event Withdraw(address indexed, address tokenType, uint256 amount);
    event SetValidators(address[], uint reward);

    // Deposit amount of tokenType
    // the pegged version of the token will be claim-able on CENNZnet
    function deposit(address tokenType, uint256 amount, bytes32 cennznetAddress) external {
        require(depositsActive, "deposits paused");
        require(IERC20(tokenType).transferFrom(msg.sender, address(this), amount), "deposit failed");

        emit Deposit(msg.sender, tokenType, amount, cennznetAddress);
    }

    // Withdraw tokens from this contract
    // Requires signatures from a threshold of current CENNZnet validators
    // v,r,s are sparse arrays expected to align w public key in 'validators'
    // i.e. v[i], r[i], s[i] matches the i-th validator[i]
    function withdraw(address tokenType, uint256 amount, uint nonce, uint8[] memory v, bytes32[] memory r, bytes32[] memory s) payable external {
        require(withdrawalsActive, "withdrawals paused");
        require(withdrawawlNonce[nonce] == false, "nonce replayed");
        require(msg.value >= withdrawalFee, "must supply withdraw fee");

        // 7769746864726177 = "withdraw"
        bytes32 digest = keccak256(abi.encodePacked(uint(0x7769746864726177), tokenType, amount, nonce));
        uint acceptanceTreshold = ((validators.length * 1000) * 51) / 1000000; // 51%
        uint32 notarizations;

        for (uint i; i < validators.length; i++) {
            // signature omitted
            if(s[i] == bytes32(0)) continue;
            // check signature
            require(validators[i] == ecrecover(digest, v[i], r[i], s[i]), "signature invalid");
            notarizations += 1;
            // have we got proven consensus?
            if(notarizations >= acceptanceTreshold) {
                break;
            }
        }

        require(notarizations >= acceptanceTreshold, "not enough signatures");
        withdrawawlNonce[nonce] = true;
        require(IERC20(tokenType).transfer(msg.sender, amount), "withdraw failed");

        emit Withdraw(msg.sender, tokenType, amount);
    }

    // Update the validator set
    // Requires signatures from a threshold of current CENNZnet validators
    // v,r,s are sparse arrays expected to align w addresses / public key in 'validators'
    // i.e. v[i], r[i], s[i] matches the i-th validator[i]
    // 6,737,588 gas
    function setValidators(
        address[] memory newValidators,
        uint32 nonce,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    ) external {
        require(nonce > validatorSetNonce, "nonce replayed");
        // 0x73657456616c696461746f7273 = "setValidators"
        bytes32 digest = keccak256(abi.encodePacked(uint(0x73657456616c696461746f7273), newValidators, nonce));

        uint acceptanceTreshold = ((validators.length * 1000) * 51) / 1000000; // 51%
        uint32 notarizations;

        for (uint i; i < validators.length; i++) {
            // signature omitted
            if(s[i] == bytes32(0)) continue;
            // check signature
            require(validators[i] == ecrecover(digest, v[i], r[i], s[i]), "signature invalid");
            notarizations += 1;
            // have we got proven consensus?
            if(notarizations >= acceptanceTreshold) {
                validators = newValidators;
                validatorSetNonce = nonce;
                break;
            }
        }

        // return any accumlated fees to the sender as a reward
        uint reward = address(this).balance;
        payable(msg.sender).transfer(reward);
        emit SetValidators(newValidators, reward);
    }

    function activateDeposits() external onlyOwner {
        depositsActive = true;
    }

    function pauseDeposits() external onlyOwner {
        depositsActive = false;
    }

    function activateWithdrawals() external onlyOwner {
        withdrawalsActive = true;
    }

    function pauseWithdrawals() external onlyOwner {
        withdrawalsActive = false;
    }

    function setWithdrawalFee(uint amount) external onlyOwner {
        withdrawalFee = amount;
    }
}