import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import {Api, SubmittableResult} from '@cennznet/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import {Contract, ContractFactory, Wallet, utils, BigNumber, ethers} from 'ethers';
import DelegateCallExploit from '../artifacts/contracts/DelegateCallExploit.sol/DelegateCallExploit.json';

use(solidity);

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}
const GENESIS_ACCOUNT = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b";

describe('GA precompiles', () => {
    let api: Api;
    let keyring: Keyring;
    let alice, bob;
    let cennznetSigner: Wallet;
    let cennznetProvider: Provider;
    let delegateCallContract: Contract;
    let cennzToken: Contract;
    const cennzTokenAddress = web3.utils.toChecksumAddress('0xCCCCCCCC00003e80000000000000000000000000');

    before(async () => {
        const providerUri = 'http://localhost:9933';
        cennznetProvider = new JsonRpcProvider(providerUri);
        cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed
        console.log(`signer address=${cennznetSigner.address}`);
        // 0x12B29179a7F858478Fde74f842126CdA5eA7AC35
        // 5EK7n4pa3FcCGoxvnJ4Qghe4xHJLJyNT6vsPWEJaSWUiTCVp

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
        console.log(`receiver address=${receiverAddress}`);

        //
        // const PRECOMPILE_PREFIXES = [
        //     1, 2, 3, 4, 5, 6, 7, 8, 9, 1024, 1025, 1026, 2048, 2049, 2050, 2051, 2052, 2053, 2054, 2055,
        // ];
        //
        // // Ethereum precompile 1-9 are pure and allowed to be called through DELEGATECALL
        // const ALLOWED_PRECOMPILE_PREFIXES = PRECOMPILE_PREFIXES.filter((add) => add <= 9);
        // const FORBIDDEN_PRECOMPILE_PREFIXES = PRECOMPILE_PREFIXES.filter((add) => add > 9);
        const DELEGATECALL_FORDIDDEN_MESSAGE =
            "0x0000000000000000000000000000000000000000000000000000000000000000" +
            "0000000000000000000000000000000000000000000000000000000000000040" +
            "000000000000000000000000000000000000000000000000000000000000002e" +
            "63616e6e6f742062652063616c6c656420" + // cannot be called
            "776974682044454c454741544543414c4c20" + // with DELEGATECALL
            "6f722043414c4c434f4445" + // or CALLCODE
            "000000000000000000000000000000000000"; // padding
        const DELEGATECALL_FORDIDDEN_MESSAGE_1 = "0x08c379a0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000087472616e73666572000000000000000000000000000000000000000000000000"
        // const contractDetails = await createContract(context, "TestCallList");
        // contractProxy = contractDetails.contract;
        // await context.createBlock({ transactions: [contractDetails.rawTx] });
        //
        // proxyInterface = new ethers.utils.Interface((await getCompiled("TestCallList")).contract.abi);

        let factory = new ContractFactory(DelegateCallExploit.abi, DelegateCallExploit.bytecode, cennznetSigner);
        delegateCallContract = await factory.deploy(cennzTokenAddress, receiverAddress);
        console.log(`contract address=${delegateCallContract.address}`);

        let tx = await delegateCallContract.trap({ gasLimit: 500000 });
        let res = await tx.wait();
        console.log(res);
        // expect(await delegateCallContract.trap({ gasLimit: 50000 })).to.equal(DELEGATECALL_FORDIDDEN_MESSAGE);

        // let tx = await delegateCallContract.trap();

        // await expect(
        //     delegateCallContract.trap().result
        // ).to.equal(DELEGATECALL_FORDIDDEN_MESSAGE);

    }).timeout(1200000000000);
});