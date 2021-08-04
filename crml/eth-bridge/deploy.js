// deploy contracts for test
async function main() {
    const Bridge = await ethers.getContractFactory('CENNZnetBridge');
    console.log('Deploying CENNZnet bridge contract...');
    const bridge = await Bridge.deploy();
    await bridge.deployed();
    console.log('CENNZnet bridge deployed to:', bridge.address);

    const TestToken = await ethers.getContractFactory('TestToken');
    console.log('Deploying TestToken contract...');
    const token = await TestToken.deploy("1000000");
    await token.deployed();
    console.log('TestToken deployed to:', token.address);

    // Make  deposit
    let depositAmount = 123;
    let cennznetAddress = "0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10";
    console.log(await bridge.activateDeposits());
    console.log(await token.approve(bridge.address, depositAmount));
    console.log(await bridge.deposit(token.address, depositAmount, cennznetAddress));
    // console.log("deposit txReceipt:", txReceipt);
    // console.log("deposit tx hash:", txReceipt.hash);
    // console.log("deposit tx data:", txReceipt.data);
    // console.log("deposit tx status:", txReceipt.status);
    //console.log(txReceipt.events?.filter((x) => {return x.event == "Deposit"}));
    // console.log(await ethers.getDefaultProvider().getTransactionReceipt(txReceipt.hash));
}

main()
    .then(() => process.exit(0))
    .catch(error => {
        console.error(error);
        process.exit(1);
    });