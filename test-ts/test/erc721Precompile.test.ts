import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import { Api } from '@cennznet/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Contract, ContractFactory, Wallet, utils, BigNumber } from 'ethers';
import ERC721PrecompileCaller from '../artifacts/contracts/Erc721PrecompileCaller.sol/Erc721PrecompileCaller.json';
import { AddressOrPair } from '@cennznet/api/types';

use(solidity);

const metadataPath = "example.com/nft/metadata";
const name = "test-collection";
const erc721Abi = [
    'event Transfer(address indexed from, address indexed to, uint256 tokenId)',
    'event Approval(address indexed owner, address indexed approved, uint256 tokenId)',
    'event ApprovalForAll(address indexed owner, address indexed operator, bool approved)',
    'function balanceOf(address who) public view returns (uint256)',
    'function ownerOf(uint256 tokenId) public view returns (address)',
    'function safeTransferFrom(address from, address to, uint256 tokenId)',
    'function transferFrom(address from, address to, uint256 tokenId)',
    'function approve(address to, uint256 tokenId)',
    'function getApproved(uint256 tokenId) public view returns (address)',
    'function setApprovalForAll(address operator, bool _approved)',
    'function isApprovedForAll(address owner, address operator) public view returns (bool)',
    'function safeTransferFrom(address from, address to, uint256 tokenId, bytes data)',
    'function name() public view returns (string memory)',
    'function symbol() public view returns (string memory)',
    'function tokenURI(uint256 tokenId) public view returns (string memory)',
];


describe('NFT precompiles', () => {
    let api: Api;
    let keyring: Keyring;
    let alice, bob: AddressOrPair;
    let cennznetSigner: Wallet;
    let cennznetProvider: Provider;
    let precompileCaller: Contract;
    let nftContract: Contract;
    let nftProxyContract: Contract;
    let globalCollectionId: any;
    // Address for NFT collection
    let nftPrecompileAddress: string = "0xAAAAAAAA00000000000000000000000000000000";
    // const cennzTokenAddress = web3.utils.toChecksumAddress('0xCCCCCCCC00003e80000000000000000000000000');

    before(async () => {
        const providerUri = 'http://localhost:9933';
        const wsProvider = 'ws://localhost:9944';
        cennznetProvider = new JsonRpcProvider(providerUri);
        cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed
        console.log(`signer address=${cennznetSigner.address}`);

        // Connect to CENNZnet local node
        await cryptoWaitReady();
        api = await Api.create({provider: wsProvider});
        keyring = new Keyring({ type: 'sr25519' });
        alice = keyring.addFromUri('//Alice');
        bob = keyring.addFromUri('//Bob');
        console.log(`Connected to CENNZnet network ${wsProvider}`);


        // Create NFT collection using runtime, bob is collection owner
        await new Promise<void>((resolve) => {
            api.tx.nft
                .createCollection(name, null)
                .signAndSend(bob, async ({status, events}) => {
                    if (status && status.isInBlock) {
                        events.forEach(({event: {data, method}}) => {
                            if (method == 'CreateCollection') {
                                globalCollectionId = (data.toJSON() as any)[0];
                                console.log(`CollectionId: ${globalCollectionId}`);
                                resolve();
                            }
                        });
                    }
                });
        });

        const collection_id_hex = (+globalCollectionId).toString(16).padStart(8, '0');
        // Assume 0 series id for first series in collection
        const series_id_hex = (+0).toString(16).padStart(8, '0');
        nftPrecompileAddress = web3.utils.toChecksumAddress(`0xAAAAAAAA${collection_id_hex}${series_id_hex}0000000000000000`);
        nftContract = new Contract(nftPrecompileAddress, erc721Abi, cennznetSigner);
        console.log(`NFT Precompile address: ${nftPrecompileAddress}`);

        await new Promise<void>((resolve) => {
            api.tx.nft.mintSeries(globalCollectionId, 10, null, metadataPath, null)
                .signAndSend(bob, async ({status, events}) => {
                    if (status && status.isInBlock) {
                        events.forEach(({event: {data, method}}) => {
                            if (method == 'CreateSeries') {
                                const collectionId = (data.toJSON() as any)[0]
                                let seriesId = (data.toJSON() as any)[1];

                                const collection_id_hex = (+collectionId).toString(16).padStart(8, '0');
                                const series_id_hex = (+seriesId).toString(16).padStart(8, '0');

                                nftPrecompileAddress = web3.utils.toChecksumAddress(`0xAAAAAAAA${collection_id_hex}${series_id_hex}0000000000000000`);
                                nftContract = new Contract(nftPrecompileAddress, erc721Abi, cennznetSigner);

                                console.log(`NFT Precompile address: ${nftPrecompileAddress}`);
                                resolve();
                            }
                        });
                    }
                });
        });
    });

    it('approve and transferFrom via EVM', async () => {
        console.log(`dev account CPAY=${(await cennznetSigner.getBalance())}`);

        let factory = new ContractFactory(ERC721PrecompileCaller.abi, ERC721PrecompileCaller.bytecode, cennznetSigner);
        precompileCaller = await factory.deploy();
        console.log(`contract address=${precompileCaller.address}`);
        const receiverAddress = await Wallet.createRandom().getAddress();
        const serial_number = 3;
        await new Promise(r => setTimeout(r, 5000));

        console.log("Approving");
        await nftContract.approve(precompileCaller.address, serial_number, {gasLimit: 50000})

        await new Promise(r => setTimeout(r, 5000));

        console.log(`Testing TransferFrom`);

        // Transfer serial_number to receiverAddress
        const transfer = await precompileCaller
            .connect(cennznetSigner)
            .transferFromProxy(nftPrecompileAddress, cennznetSigner.address, receiverAddress, serial_number, {gasLimit: 50000});
        await transfer.wait()

        // contract_address now owner of serial_number
        expect(
            await precompileCaller.balanceOfProxy(receiverAddress)
        ).to.equal(1);
        expect(
            await precompileCaller.ownerOfProxy(serial_number)
        ).to.equal(receiverAddress);


    }).timeout(100000000000000);

});

