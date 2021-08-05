// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";

contract CENNZnetBridge is Ownable {
    using SafeMath for uint256;

    // whether the bridge is accepting deposits & withdrawals
    bool isActive;
    // map from integer index to validator ECDSA address
    // this is the CENNZnet validator ECDSA session keys
    address[] public validators;
    // Nonce for validator set changes
    uint32 validatorsNonce;
    // withdrawal nonces
    mapping(uint => bool) public withdrawNonce;
    mapping(address => uint) public trustedWithdrawNonces;
    // withdraw authority
    address public withdrawAuthority;

    event Deposit(address indexed, address tokenType, uint256 amount, bytes32 cennznetAddress, uint256 timestamp);
    event Withdraw(address indexed, address tokenType, uint256 amount);

    // Deposit amount of tokenType
    // the pegged version of the token will be claim-able on CENNZnet
    function deposit(address tokenType, uint256 amount, bytes32 cennznetAddress) nonRentrant external {
        require(isActive, "deposits paused");
        require(IERC20(tokenType).transferFrom(msg.sender, address(this), amount), "deposit failed");

        emit Deposit(msg.sender, tokenType, amount, cennznetAddress, block.timestamp);
    }

    // Withdraw must be signed by withdraw authority and owner of funds
    // user deposits a signed ga deposit payload
    // ecc recover to check it's message sender
    // by publishing signed tx anyone can ensure they've actually claimed the funds by submitting themselves...
    function trustedWithdraw(address tokenType, uint256 amount, uint nonce, uint8 v, bytes32 r, bytes32 s) {
        require(isActive, "withdrawals paused");
        require(trustedWithdrawNonces[msg.sender] < nonce, "old nonce");

        // check sig
        // 7769746864726177 = "withdraw"
        bytes32 digest = keccak256(abi.encodePacked(0x7769746864726177, msg.sender, tokenType, amount, nonce));
        require(withdrawAuthority == ecrecover(digest, v, r, s), "signature invalid");
        trustedWithdrawNonces[msg.sender] = nonce;

        IERC20(tokenType).transfer(address(this), msg.sender, amount);
    }

    // Withdraw tokens from this contract
    // Requires signatures from a threshold of current CENNZnet validators
    // v,r,s are sparse arrays expected to align w public key in 'validators'
    // i.e. v[i], r[i], s[i] matches the i-th validator[i]
    function withdraw(address tokenType, uint256 amount, uint nonce, uint8[] memory v, bytes32[] memory r, bytes32[] memory s) nonRentrant external {
        require(isActive, "withdrawals paused");
        require(withdrawNonce[nonce] == false, "nonce replayed");

        // 7769746864726177 = "withdraw"
        bytes32 digest = keccak256(abi.encodePacked(0x7769746864726177, tokenType, amount, nonce));
        uint acceptanceTreshold = ((validators.length * 1000) * 667) / 1000000; // 2/3rds
        uint notarizations;

        for (uint i; i < validators.length; i++) {
            // signature omitted
            if(s[i] == bytes32(0)) continue;
            // check signature
            require(validators[i] == ecrecover(digest, v[i], r[i], s[i]), "signature invalid");
            notarizations += 1;
            // have we got proven consensus?
            if(notariazations >= acceptanceTreshold) {
                break;
            }
        }

        require(notariazations >= acceptanceTreshold, "not enough signatures");
        require(IERC20(tokenType).transfer(address(this), msg.sender, amount), "withdraw failed");
        withdrawNonce[nonce] = true;

        emit Withdraw(msg.sender, tokenType, amount);
    }

    // Update the validator set
    // Requires signatures from a threshold of current CENNZnet validators
    // v,r,s are sparse arrays expected to align w public key in 'validators'
    // i.e. v[i], r[i], s[i] matches the i-th validator[i]
    // we could only update this if the set changes in a big way
    function updateValidators(
        address[] memory newValidators,
        uint32 nonce,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    ) nonRentrant external {
        require(nonce > validatorsNonce, "nonce replayed");
        // TODO: investigate gas cost to hash this
        // 757064617465 = "update"
        bytes32 digest = keccak256(abi.encodePacked(0x757064617465, newValidators, nonce));

        uint acceptanceTreshold = ((validators.length * 1000) * 667) / 1000000; // 2/3rds
        uint32 notarizations;

        for (uint i; i < validators.length; i++) {
            // signature omitted
            if(s[i] == bytes32(0)) continue;
            // check signature
            require(validators[i] == ecrecover(digest, v[i], r[i], s[i]), "signature invalid");
            notarizations += 1;
            // have we got proven consensus?
            if(notariazations >= acceptanceTreshold) {
                validators = newValidators;
                validatorsNonce = nonce;
                break;
            }
        }
        // TODO: drop an event
    }

    function activate() external onlyOwner {
        isActive = true;
    }

    function pause() external onlyOwner {
        isActive = false;
    }
}