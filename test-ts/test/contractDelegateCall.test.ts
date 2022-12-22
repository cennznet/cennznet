import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import {Api} from '@cennznet/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import {Contract, ContractFactory, Wallet} from 'ethers';
import DelegateCallExploit from '../artifacts/contracts/DelegateCallExploit.sol/DelegateCallExploit.json';

use(solidity);

describe('GA precompiles', () => {
    let api: Api;
    let keyring: Keyring;
    let alice, bob;
    let cennznetSigner: Wallet;
    const cennznetSignerSS58 = '5EK7n4pa3FcCGoxvnJ4Qghe4xHJLJyNT6vsPWEJaSWUiTCVp';
    let cennznetProvider: Provider;
    let delegateCallContract: Contract;
    const cennzTokenAddress = web3.utils.toChecksumAddress('0xCCCCCCCC00003e80000000000000000000000000');

    before(async () => {
        const providerUri = 'http://localhost:9933';
        cennznetProvider = new JsonRpcProvider(providerUri);
        cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed


        // Connect to CENNZnet local node
        await cryptoWaitReady();
        api = await Api.create({provider: providerUri});
        keyring = new Keyring({ type: 'sr25519' });
        alice = keyring.addFromUri('//Alice');
        bob = keyring.addFromUri('//Bob');
        console.log(`Connected to CENNZnet network ${providerUri}`);

    });

    it('DELEGATE call for precompiles', async () => {
        const receiverAddress = await Wallet.createRandom().getAddress();
        let factory = new ContractFactory(DelegateCallExploit.abi, DelegateCallExploit.bytecode, cennznetSigner);
        delegateCallContract = await factory.deploy(cennzTokenAddress, receiverAddress);
        await delegateCallContract.deployTransaction.wait();

        // Get balance of CENNZ before trap is called
        const balance_before = await api.query.genericAsset.freeBalance(16000, cennznetSignerSS58);

        const tx = await delegateCallContract.connect(cennznetSigner).trap({ gasLimit: 300000 });
        await expect(tx.wait()).to.be.reverted;

        // Get balance of CENNZ after trap is called
        const balance_after = await api.query.genericAsset.freeBalance(16000, cennznetSignerSS58);

        // Balance shouldn't have changed as the trap was unsuccessful
        expect(balance_before.toString()).to.eq(balance_after.toString());
    }).timeout(12000);
});