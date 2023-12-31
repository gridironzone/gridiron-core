import {strictEqual} from "assert"
import {Gridiron} from "./lib.js";
import {
    NativeAsset,
    newClient,
    readArtifact,
    TokenAsset,
} from "../helpers.js"


async function main() {
    const { terra, wallet } = newClient()
    const network = readArtifact(terra.config.chainID)

    const gridiron = new Gridiron(terra, wallet);
    console.log(`chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`)

    // 1. Provide liquidity
    await provideLiquidity(network, gridiron, wallet.key.accAddress)

    // 2. Stake GRID
    await stake(network, gridiron, wallet.key.accAddress)

    // 3. Swap tokens in pool
    await swap(network, gridiron, wallet.key.accAddress)

    // 4. Collect Maker fees
    await collectFees(network, gridiron, wallet.key.accAddress)

    // 5. Withdraw liquidity
    await withdrawLiquidity(network, gridiron, wallet.key.accAddress)

    // 6. Unstake GRID
    await unstake(network, gridiron, wallet.key.accAddress)
}

async function provideLiquidity(network: any, gridiron: Gridiron, accAddress: string) {
    const liquidity_amount = 100000000;
    const pool_uust_grid = gridiron.pair(network.poolGridUst);

    // Provide liquidity in order to swap
    await pool_uust_grid.provideLiquidity(new NativeAsset('uusd', liquidity_amount.toString()), new TokenAsset(network.tokenAddress, liquidity_amount.toString()))

    let grid_balance = await gridiron.getTokenBalance(network.tokenAddress, accAddress);
    let xgrid_balance = await gridiron.getTokenBalance(network.xgridAddress, accAddress);

    console.log(`GRID balance: ${grid_balance}`)
    console.log(`xGRID balance: ${xgrid_balance}`)
}

async function withdrawLiquidity(network: any, gridiron: Gridiron, accAddress: string) {
    const pool_uust_grid = gridiron.pair(network.poolGridUst);

    let pair_info = await pool_uust_grid.queryPair();
    let lp_token_amount = await gridiron.getTokenBalance(pair_info.liquidity_token, accAddress);

    // Withdraw liquidity
    await pool_uust_grid.withdrawLiquidity(pair_info.liquidity_token, lp_token_amount.toString());

    let grid_balance = await gridiron.getTokenBalance(network.tokenAddress, accAddress);
    let xgrid_balance = await gridiron.getTokenBalance(network.xgridAddress, accAddress);

    console.log(`GRID balance: ${grid_balance}`)
    console.log(`xGRID balance: ${xgrid_balance}`)
}

async function stake(network: any, gridiron: Gridiron, accAddress: string) {
    let grid_balance = await gridiron.getTokenBalance(network.tokenAddress, accAddress);
    let xgrid_balance = await gridiron.getTokenBalance(network.xgridAddress, accAddress);

    const staking = gridiron.staking(network.stakingAddress);
    const staking_amount = 100000;

    console.log(`Staking ${staking_amount} GRID`)
    await staking.stakeGrid(network.tokenAddress, staking_amount.toString())

    let new_grid_balance = await gridiron.getTokenBalance(network.tokenAddress, accAddress);
    let new_xgrid_balance = await gridiron.getTokenBalance(network.xgridAddress, accAddress);

    console.log(`GRID balance: ${new_grid_balance}`)
    console.log(`xGRID balance: ${new_xgrid_balance}`)

    strictEqual(true, new_grid_balance < grid_balance);
    strictEqual(true, new_xgrid_balance > xgrid_balance);
}

async function unstake(network: any, gridiron: Gridiron, accAddress: string) {
    let grid_balance = await gridiron.getTokenBalance(network.tokenAddress, accAddress);
    let xgrid_balance = await gridiron.getTokenBalance(network.xgridAddress, accAddress);

    const staking = gridiron.staking(network.stakingAddress);

    console.log(`Unstaking ${xgrid_balance} xGRID`)
    await staking.unstakeGrid(network.xgridAddress, xgrid_balance.toString())

    let final_grid_balance = await gridiron.getTokenBalance(network.tokenAddress, accAddress);
    let final_xgrid_balance = await gridiron.getTokenBalance(network.xgridAddress, accAddress);

    console.log(`GRID balance: ${final_grid_balance}`)
    console.log(`xGRID balance: ${final_xgrid_balance}`)

    strictEqual(true, final_grid_balance >= grid_balance);
    strictEqual(final_xgrid_balance, 0);
}

async function swap(network: any, gridiron: Gridiron, accAddress: string) {
    const pool_uust_grid = gridiron.pair(network.poolGridUst);
    const factory = gridiron.factory(network.factoryAddress);
    const swap_amount = 10000;

    let pair_info = await pool_uust_grid.queryPair();

    let grid_balance = await gridiron.getTokenBalance(network.tokenAddress, accAddress);
    let xgrid_balance = await gridiron.getTokenBalance(network.xgridAddress, accAddress);

    console.log(`GRID balance: ${grid_balance}`)
    console.log(`xGRID balance: ${xgrid_balance}`)

    let fee_info = await factory.queryFeeInfo('xyk');
    strictEqual(true,  fee_info.fee_address != null, "fee address is not set")
    strictEqual(true,  fee_info.total_fee_bps > 0, "total_fee_bps address is not set")
    strictEqual(true,  fee_info.maker_fee_bps > 0, "maker_fee_bps address is not set")

    console.log('swap some tokens back and forth to accumulate commission')
    for (let index = 0; index < 5; index++) {
        console.log("swap grid to uusd")
        await pool_uust_grid.swapCW20(network.tokenAddress, swap_amount.toString())

        console.log("swap uusd to grid")
        await pool_uust_grid.swapNative(new NativeAsset('uusd', swap_amount.toString()))

        let lp_token_amount = await gridiron.getTokenBalance(pair_info.liquidity_token, accAddress);
        let share_info = await pool_uust_grid.queryShare(lp_token_amount.toString());
        console.log(share_info)
    }
}

async function collectFees(network: any, gridiron: Gridiron, accAddress: string) {
    const maker = gridiron.maker(network.makerAddress);

    let maker_cfg = await maker.queryConfig();
    strictEqual(maker_cfg.grid_token_contract, network.tokenAddress)
    strictEqual(maker_cfg.staking_contract, network.stakingAddress)

    let balances = await maker.queryBalances([new TokenAsset(network.tokenAddress, '0')]);
    strictEqual(true, balances.length > 0, "maker balances are empty. no fees are collected")

    console.log(balances)

    let resp = await maker.collect([network.poolGridUst])
    console.log(resp)
}

main().catch(console.log)
