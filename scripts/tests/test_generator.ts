import {Gridiron, Generator} from "./lib.js";
import {provideLiquidity} from "./test_router.js"
import {
    NativeAsset,
    newClient,
    readArtifact, TokenAsset,
} from "../helpers.js"

async function main() {
    const cl = newClient()
    const network = readArtifact(cl.terra.config.chainID)

    const gridiron = new Gridiron(cl.terra, cl.wallet);
    console.log(`chainID: ${cl.terra.config.chainID} wallet: ${cl.wallet.key.accAddress}`)

    // 1. Provide GRID-UST liquidity
    const liquidity_amount = 5000000;
    await provideLiquidity(network, gridiron, cl.wallet.key.accAddress, network.poolGridUst, [
        new NativeAsset('uusd', liquidity_amount.toString()),
        new TokenAsset(network.tokenAddress, liquidity_amount.toString())
    ])

    // 2. Provide LUNA-UST liquidity
    await provideLiquidity(network, gridiron, cl.wallet.key.accAddress, network.poolLunaUst, [
        new NativeAsset('uluna', liquidity_amount.toString()),
        new NativeAsset('uusd', liquidity_amount.toString())
    ])

    // 3. Fetch the pool balances
    let lpTokenGridUst = await gridiron.getTokenBalance(network.lpTokenGridUst, cl.wallet.key.accAddress);
    let lpTokenLunaUst = await gridiron.getTokenBalance(network.lpTokenLunaUst, cl.wallet.key.accAddress);

    console.log(`GridUst balance: ${lpTokenGridUst}`)
    console.log(`LunaUst balance: ${lpTokenLunaUst}`)

    const generator = gridiron.generator(network.generatorAddress);
    console.log("generator config: ", await generator.queryConfig());

    // 4. Register generators
    await generator.registerGenerator([
        [network.lpTokenGridUst, "24528"],
        [network.lpTokenLunaUst, "24528"],
    ])

    // 4. Deposit to generator
    await generator.deposit(network.lpTokenGridUst, "623775")
    await generator.deposit(network.lpTokenLunaUst, "10000000")

    // 5. Fetch the deposit balances
    console.log(`deposited: ${await generator.queryDeposit(network.lpTokenGridUst, cl.wallet.key.accAddress)}`)
    console.log(`deposited: ${await generator.queryDeposit(network.lpTokenLunaUst, cl.wallet.key.accAddress)}`)

    // 6. Find checkpoint generators limit for user boost
    await findCheckpointGeneratorsLimit(generator, network)
}

async function findCheckpointGeneratorsLimit(generator: Generator, network: any) {
    let generators = []
    for(let i = 0; i < 40; i++) {
        generators.push(network.lpTokenGridUst)
        generators.push(network.lpTokenLunaUst)
    }

    await generator.checkpointUserBoost(generators)

}

main().catch(console.log)
