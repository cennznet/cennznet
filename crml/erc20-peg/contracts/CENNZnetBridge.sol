// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/access/Ownable.sol";

// Provides methods for verifying messages from the CENNZnet validator set
contract CENNZnetBridge is Ownable {
    // map from validator set nonce to validator ECDSA addresses (i.e bridge session keys)
    // these should be in sorted order matching `pallet_session::Module<T>::validators()`
    // signatures from a threshold of these addresses are considered approved by the CENNZnet protocol
    mapping(uint => address[]) public validators;
    // Nonce for validator set changes
    uint32 validatorSetId;
    // Message nonces.
    // CENNZnet will only validate one message per nonce.
    // Claiming out of order is ok.
    mapping(uint => bool) public eventIds;
    // Fee for CENNZnet message verification
    // Offsets bridge upkeep costs i.e updating the validator set
    uint verificationFee = 1e15;

    event SetValidators(address[], uint reward);

    // Verify a message was authorised by CENNZnet validators.
    // Callable by anyone.
    // Caller must provide `verificationFee`.
    // Requires signatures from a threshold of current CENNZnet validators.
    // Halts on failure
    //
    // Parameters:
    // - message: encoded arguments (packed) e.g. `abi.encodePacked(arg0, arg1, argN);` (NO nonce)
    // - nonce: the message nonce as given by CENNZnet
    // - v,r,s are sparse arrays expected to align w public key in 'validators'
    // i.e. v[i], r[i], s[i] matches the i-th validator[i]
    function verifyMessage(bytes memory message, uint256 event_id, uint8[] memory v, bytes32[] memory r, bytes32[] memory s) payable external {
        require(!eventIds[event_id], "event_id replayed");
        require(msg.value >= verificationFee || msg.sender == address(this), "must supply withdraw fee");

        bytes32 digest = keccak256(abi.encodePacked(message, event_id));
        uint acceptanceTreshold = ((validators[event_id].length * 1000) * 51) / 1000000; // 51%
        uint32 notarizations;

        for (uint i; i < validators[event_id].length; i++) {
            // signature omitted
            if(s[i] == bytes32(0)) continue;
            // check signature
            require(validators[event_id][i] == ecrecover(digest, v[i], r[i], s[i]), "signature invalid");
            notarizations += 1;
            // have we got proven consensus?
            if(notarizations >= acceptanceTreshold) {
                break;
            }
        }

        require(notarizations >= acceptanceTreshold, "not enough signatures");
        eventIds[event_id] = true;
    }

    // Update the known CENNZnet validator set
    //
    // Requires signatures from a threshold of current CENNZnet validators
    // v,r,s are sparse arrays expected to align w addresses / public key in 'validators'
    // i.e. v[i], r[i], s[i] matches the i-th validator[i]
    // ~6,737,588 gas
    function setValidators(
        address[] memory newValidators,
        uint32 validatorSetId,
        uint event_id,
        uint8[] memory v,
        bytes32[] memory r,
        bytes32[] memory s
    ) external {
        require(validatorSetId > validatorSetId, "validator set id replayed");

        // 0x73657456616c696461746f7273 = "setValidators"
        bytes memory message = abi.encodePacked(uint(0x73657456616c696461746f7273), newValidators, validatorSetId);
        verifyMessage(message, event_id, v, r, s);

        // update
        validators[validatorSetId] = newValidators;
        validatorSetId = validatorSetId;

        // return any accumlated fees to the sender as a reward
        uint reward = address(this).balance;
        payable(msg.sender).transfer(reward);

        emit SetValidators(newValidators, reward);
    }
}
