#![cfg(not(tarpaulin_include))]

use std::cell::RefCell;
use std::rc::Rc;

use cosmwasm_std::{coins, Addr, Coin, Decimal, Decimal256, StdError, Uint128};
use injective_cosmwasm::InjectiveQuerier;
use injective_testing::generate_inj_address;
use itertools::{max, Itertools};

use gridiron::asset::{native_asset_info, AssetInfoExt, MINIMUM_LIQUIDITY_AMOUNT};
use gridiron::cosmwasm_ext::{AbsDiff, IntegerToDecimal};
use gridiron::factory::PairType;
use gridiron::pair_concentrated::{
    ConcentratedPoolParams, ConcentratedPoolUpdateParams, PromoteParams, UpdatePoolParams,
};
use gridiron::pair_concentrated_inj::{ExecuteMsg, MigrateMsg, OrderbookConfig};
use gridiron_mocks::cw_multi_test::Executor;
use gridiron_pair_concentrated_injective::error::ContractError;
use gridiron_pair_concentrated_injective::orderbook::consts::MIN_TRADES_TO_AVG_LIMITS;
use gridiron_pcl_common::consts::{AMP_MAX, AMP_MIN, MA_HALF_TIME_LIMITS};
use gridiron_pcl_common::error::PclError;

use crate::helper::mocks::{mock_inj_app, InjAppExt, MockFundingMode};
use crate::helper::{
    common_pcl_params, dec_to_f64, f64_to_dec, orderbook_pair_contract, AppExtension, Helper,
    TestCoin,
};

mod helper;

#[test]
fn check_wrong_initialization() {
    let owner = Addr::unchecked("owner");

    let params = common_pcl_params();

    let mut wrong_params = params.clone();
    wrong_params.amp = Decimal::zero();

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::native("GRID")],
        wrong_params,
        true,
    )
    .unwrap_err();

    assert_eq!(
        ContractError::PclError(PclError::IncorrectPoolParam(
            "amp".to_string(),
            AMP_MIN.to_string(),
            AMP_MAX.to_string()
        )),
        err.downcast().unwrap(),
    );

    let mut wrong_params = params.clone();
    wrong_params.ma_half_time = MA_HALF_TIME_LIMITS.end() + 1;

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::native("GRID")],
        wrong_params,
        true,
    )
    .unwrap_err();

    assert_eq!(
        ContractError::PclError(PclError::IncorrectPoolParam(
            "ma_half_time".to_string(),
            MA_HALF_TIME_LIMITS.start().to_string(),
            MA_HALF_TIME_LIMITS.end().to_string()
        )),
        err.downcast().unwrap(),
    );

    let mut wrong_params = params.clone();
    wrong_params.price_scale = Decimal::zero();

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::native("GRID")],
        wrong_params,
        true,
    )
    .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Initial price scale can not be zero",
    );

    // check instantiation with valid params
    Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::native("GRID")],
        params,
        true,
    )
    .unwrap();
}

#[test]
fn provide_and_withdraw() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("USDC")];

    let params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params, true).unwrap();

    // checking LP token virtual price on an empty pool
    let lp_price = helper.query_lp_price().unwrap();
    assert!(
        lp_price.is_zero(),
        "LP price must be zero before any provide"
    );

    let user1 = Addr::unchecked("user1");

    // Try to provide with wrong asset
    let random_coin = native_asset_info("random-coin".to_string()).with_balance(100u8);
    let wrong_assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        random_coin.clone(),
    ];
    helper.give_me_money(&wrong_assets, &user1);
    let err = helper.provide_liquidity(&user1, &wrong_assets).unwrap_err();
    assert_eq!(
        "Generic error: Unexpected asset random-coin",
        err.root_cause().to_string()
    );

    // Provide with asset which does not belong to the pair
    let err = helper
        .provide_liquidity(
            &user1,
            &[
                random_coin.clone(),
                helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
            ],
        )
        .unwrap_err();
    assert_eq!(
        "Generic error: Unexpected asset random-coin",
        err.root_cause().to_string()
    );

    let err = helper
        .provide_liquidity(&user1, &[random_coin])
        .unwrap_err();
    assert_eq!(
        "The asset random-coin does not belong to the pair",
        err.root_cause().to_string()
    );

    let err = helper.provide_liquidity(&user1, &[]).unwrap_err();
    assert_eq!(
        "Generic error: Nothing to provide",
        err.root_cause().to_string()
    );

    helper.give_me_money(
        &[helper.assets[&test_coins[1]].with_balance(50_000_000000u128)],
        &user1,
    );

    // Try to provide with zero amount
    let err = helper
        .provide_liquidity(
            &user1,
            &[
                helper.assets[&test_coins[0]].with_balance(0u8),
                helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
            ],
        )
        .unwrap_err();
    assert_eq!(ContractError::InvalidZeroAmount {}, err.downcast().unwrap());

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
    ];
    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(70710_677118, helper.token_balance(&helper.lp_token, &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));
    assert_eq!(
        helper
            .query_share(helper.token_balance(&helper.lp_token, &user1))
            .unwrap(),
        vec![
            helper.assets[&test_coins[0]].with_balance(99999998584u128),
            helper.assets[&test_coins[1]].with_balance(49999999292u128)
        ]
    );

    let user2 = Addr::unchecked("user2");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
    ];
    helper.give_me_money(&assets, &user2);
    helper.provide_liquidity(&user2, &assets).unwrap();
    assert_eq!(
        70710_677118 + MINIMUM_LIQUIDITY_AMOUNT.u128(),
        helper.token_balance(&helper.lp_token, &user2)
    );

    // Changing order of assets does not matter
    let user3 = Addr::unchecked("user3");
    let assets = vec![
        helper.assets[&test_coins[1]].with_balance(50_000_000000u128),
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
    ];
    helper.give_me_money(&assets, &user3);
    helper.provide_liquidity(&user3, &assets).unwrap();
    assert_eq!(
        70710_677118 + MINIMUM_LIQUIDITY_AMOUNT.u128(),
        helper.token_balance(&helper.lp_token, &user3)
    );

    // After initial provide one-sided provide is allowed
    let user4 = Addr::unchecked("user4");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(0u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.give_me_money(&assets, &user4);
    helper.provide_liquidity(&user4, &assets).unwrap();
    // LP amount is less than for prev users as provide is imbalanced
    assert_eq!(62217_722016, helper.token_balance(&helper.lp_token, &user4));

    // One of assets may be omitted
    let user5 = Addr::unchecked("user5");
    let assets = vec![helper.assets[&test_coins[0]].with_balance(140_000_000000u128)];
    helper.give_me_money(&assets, &user5);
    helper.provide_liquidity(&user5, &assets).unwrap();
    assert_eq!(57271_023590, helper.token_balance(&helper.lp_token, &user5));

    // check that imbalanced withdraw is currently disabled
    let withdraw_assets = vec![
        helper.assets[&test_coins[0]].with_balance(10_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(5_000_000000u128),
    ];
    let err = helper
        .withdraw_liquidity(&user1, 7071_067711, withdraw_assets)
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Imbalanced withdraw is currently disabled"
    );

    // user1 withdraws 1/10 of his LP tokens
    helper
        .withdraw_liquidity(&user1, 7071_067711, vec![])
        .unwrap();

    assert_eq!(
        70710_677118 - 7071_067711,
        helper.token_balance(&helper.lp_token, &user1)
    );
    assert_eq!(9382_010960, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(5330_688045, helper.coin_balance(&test_coins[1], &user1));

    // user2 withdraws half
    helper
        .withdraw_liquidity(&user2, 35355_339059, vec![])
        .unwrap();

    assert_eq!(
        70710_677118 + MINIMUM_LIQUIDITY_AMOUNT.u128() - 35355_339059,
        helper.token_balance(&helper.lp_token, &user2)
    );
    assert_eq!(46910_055478, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(26653_440612, helper.coin_balance(&test_coins[1], &user2));
}

#[test]
fn check_imbalanced_provide() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("USDC")];

    let mut params = ConcentratedPoolParams {
        price_scale: Decimal::from_ratio(2u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params.clone(), true).unwrap();

    let user1 = Addr::unchecked("user1");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.give_me_money(&assets, &user1);
    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(
        100285_256937,
        helper.token_balance(&helper.lp_token, &user1)
    );
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));

    // creating a new pool with inverted price scale
    params.price_scale = Decimal::from_ratio(1u8, 2u8);

    let mut helper = Helper::new(&owner, test_coins.clone(), params, true).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.give_me_money(&assets, &user1);
    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(
        100285_256937,
        helper.token_balance(&helper.lp_token, &user1)
    );
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));
}

#[test]
fn provide_with_different_precision() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("FOO"), TestCoin::native("BAR")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params(), true).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_00000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];

    helper.provide_liquidity(&owner, &assets).unwrap();

    let tolerance = 9;

    for user_name in ["user1", "user2", "user3"] {
        let user = Addr::unchecked(user_name);

        helper.give_me_money(&assets, &user);

        helper.provide_liquidity(&user, &assets).unwrap();

        let lp_amount = helper.token_balance(&helper.lp_token, &user);
        assert!(
            100_000000 - lp_amount < tolerance,
            "LP token balance assert failed for {user}"
        );
        assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
        assert_eq!(0, helper.coin_balance(&test_coins[1], &user));

        helper.withdraw_liquidity(&user, lp_amount, vec![]).unwrap();

        assert_eq!(0, helper.token_balance(&helper.lp_token, &user));
        assert!(
            100_00000 - helper.coin_balance(&test_coins[0], &user) < tolerance,
            "Withdrawn amount of coin0 assert failed for {user}"
        );
        assert!(
            100_000000 - helper.coin_balance(&test_coins[1], &user) < tolerance,
            "Withdrawn amount of coin1 assert failed for {user}"
        );
    }
}

#[test]
fn swap_different_precisions() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("FOO"), TestCoin::native("BAR")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params(), true).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_00000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let user = Addr::unchecked("user");
    // 100 x FOO tokens
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_00000u128);

    // Checking direct swap simulation
    let sim_resp = helper.simulate_swap(&offer_asset, None).unwrap();
    // And reverse swap as well
    let reverse_sim_resp = helper
        .simulate_reverse_swap(
            &helper.assets[&test_coins[1]].with_balance(sim_resp.return_amount.u128()),
            None,
        )
        .unwrap();
    assert_eq!(reverse_sim_resp.offer_amount.u128(), 10019003);
    assert_eq!(reverse_sim_resp.commission_amount.u128(), 45084);
    assert_eq!(reverse_sim_resp.spread_amount.u128(), 125);

    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    // 99_737929 x BAR tokens
    assert_eq!(99_737929, sim_resp.return_amount.u128());
    assert_eq!(
        sim_resp.return_amount.u128(),
        helper.coin_balance(&test_coins[1], &user)
    );
}

#[test]
fn check_reverse_swap() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("uusd")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params(), true).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let offer_asset = helper.assets[&test_coins[0]].with_balance(50_000_000000u128);

    let sim_resp = helper.simulate_swap(&offer_asset, None).unwrap();
    let reverse_sim_resp = helper
        .simulate_reverse_swap(
            &helper.assets[&test_coins[1]].with_balance(sim_resp.return_amount.u128()),
            None,
        )
        .unwrap();
    assert_eq!(reverse_sim_resp.offer_amount.u128(), 50000220879u128); // as it is hard to predict dynamic fees reverse swap is not exact
    assert_eq!(reverse_sim_resp.commission_amount.u128(), 151_913981);
    assert_eq!(reverse_sim_resp.spread_amount.u128(), 16241_558397);
}

#[test]
fn check_swaps_simple() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("USDC")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params(), true).unwrap();

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);

    // Check swap does not work if pool is empty
    let err = helper.swap(&user, &offer_asset, None).unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: One of the pools is empty"
    );

    // Try to swap a wrong asset
    let wrong_coin = native_asset_info("random-coin".to_string());
    let wrong_asset = wrong_coin.with_balance(100_000000u128);
    helper.give_me_money(&[wrong_asset.clone()], &user);
    let err = helper.swap(&user, &wrong_asset, None).unwrap_err();
    assert_eq!(
        ContractError::InvalidAsset(wrong_coin.to_string()),
        err.downcast().unwrap()
    );

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let d = helper.query_d().unwrap();
    assert_eq!(dec_to_f64(d), 200000f64);

    helper.swap(&user, &offer_asset, None).unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    assert_eq!(99_737929, helper.coin_balance(&test_coins[1], &user));

    let offer_asset = helper.assets[&test_coins[0]].with_balance(90_000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    let err = helper.swap(&user, &offer_asset, None).unwrap_err();
    assert_eq!(
        ContractError::PclError(PclError::MaxSpreadAssertion {}),
        err.downcast().unwrap()
    );

    let user2 = Addr::unchecked("user2");
    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user2);
    helper.swap(&user2, &offer_asset, None).unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user2));
    assert_eq!(99_741246, helper.coin_balance(&test_coins[0], &user2));

    let d = helper.query_d().unwrap();
    assert_eq!(dec_to_f64(d), 200000.260415);

    let price1 = helper.observe_price(0).unwrap();
    helper.app.next_block(10);
    // Swapping the lowest amount possible which results in positive return amount
    helper
        .swap(
            &user,
            &helper.assets[&test_coins[1]].with_balance(2u128),
            None,
        )
        .unwrap();
    let price2 = helper.observe_price(0).unwrap();
    // With such a small swap size contract doesn't store observation
    assert_eq!(price1, price2);

    helper.app.next_block(10);
    // Swap the smallest possible amount which gets observation saved
    helper
        .swap(
            &user,
            &helper.assets[&test_coins[1]].with_balance(1005u128),
            None,
        )
        .unwrap();
    let price3 = helper.observe_price(0).unwrap();
    // Prove that price didn't jump that much
    let diff = price3.diff(price2);
    assert!(
        diff / price2 < f64_to_dec(0.005),
        "price jumped from {price2} to {price3} which is more than 0.5%"
    );
}

#[test]
fn check_swaps_with_price_update() {
    let owner = Addr::unchecked("owner");
    let half = Decimal::from_ratio(1u8, 2u8);

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("USDC")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params(), true).unwrap();

    helper.app.next_block(1000);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper.app.next_block(1000);

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[1]].with_balance(10_000_000000u128);
    let mut prev_vlp_price = helper.query_lp_price().unwrap();

    for i in 0..4 {
        helper.give_me_money(&[offer_asset.clone()], &user1);
        helper.swap(&user1, &offer_asset, Some(half)).unwrap();
        let new_vlp_price = helper.query_lp_price().unwrap();
        assert!(
            new_vlp_price >= prev_vlp_price,
            "{i}: new_vlp_price <= prev_vlp_price ({new_vlp_price} <= {prev_vlp_price})",
        );
        prev_vlp_price = new_vlp_price;
        helper.app.next_block(1000);
    }

    let offer_asset = helper.assets[&test_coins[0]].with_balance(10_000_000000u128);
    for _i in 0..4 {
        helper.give_me_money(&[offer_asset.clone()], &user1);
        helper.swap(&user1, &offer_asset, Some(half)).unwrap();
        helper.app.next_block(1000);
    }
}

#[test]
fn provides_and_swaps() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("USDC")];

    let mut helper = Helper::new(&owner, test_coins.clone(), common_pcl_params(), true).unwrap();

    helper.app.next_block(1000);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper.app.next_block(1000);

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    let provider = Addr::unchecked("provider");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(1_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000_000000u128),
    ];
    helper.give_me_money(&assets, &provider);
    helper.provide_liquidity(&provider, &assets).unwrap();

    let offer_asset = helper.assets[&test_coins[1]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();

    helper
        .withdraw_liquidity(&provider, 999_999354, vec![])
        .unwrap();

    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);
    helper.swap(&user, &offer_asset, None).unwrap();
}

#[test]
fn check_amp_gamma_change() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uluna"), TestCoin::native("USDC")];

    let params = ConcentratedPoolParams {
        amp: f64_to_dec(40f64),
        gamma: f64_to_dec(0.0001),
        ..common_pcl_params()
    };
    let mut helper = Helper::new(&owner, test_coins, params, true).unwrap();

    let random_user = Addr::unchecked("random");
    let action = ConcentratedPoolUpdateParams::Update(UpdatePoolParams {
        mid_fee: Some(f64_to_dec(0.002)),
        out_fee: None,
        fee_gamma: None,
        repeg_profit_threshold: None,
        min_price_scale_delta: None,
        ma_half_time: None,
    });

    let err = helper.update_config(&random_user, &action).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err.downcast().unwrap());

    helper.update_config(&owner, &action).unwrap();

    helper.app.next_block(86400);

    let future_time = helper.app.block_info().time.seconds() + 100_000;
    let target_amp = 44f64;
    let target_gamma = 0.00009;
    let action = ConcentratedPoolUpdateParams::Promote(PromoteParams {
        next_amp: f64_to_dec(target_amp),
        next_gamma: f64_to_dec(target_gamma),
        future_time,
    });
    helper.update_config(&owner, &action).unwrap();

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 40f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.0001);
    assert_eq!(amp_gamma.future_time, future_time);

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 42f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.000095);
    assert_eq!(amp_gamma.future_time, future_time);

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), target_amp);
    assert_eq!(dec_to_f64(amp_gamma.gamma), target_gamma);
    assert_eq!(amp_gamma.future_time, future_time);

    // change values back
    let future_time = helper.app.block_info().time.seconds() + 100_000;
    let action = ConcentratedPoolUpdateParams::Promote(PromoteParams {
        next_amp: f64_to_dec(40f64),
        next_gamma: f64_to_dec(0.000099),
        future_time,
    });
    helper.update_config(&owner, &action).unwrap();

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 42f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.0000945);
    assert_eq!(amp_gamma.future_time, future_time);

    // stop changing amp and gamma thus fixing current values
    let action = ConcentratedPoolUpdateParams::StopChangingAmpGamma {};
    helper.update_config(&owner, &action).unwrap();
    let amp_gamma = helper.query_amp_gamma().unwrap();
    let last_change_time = helper.app.block_info().time.seconds();
    assert_eq!(amp_gamma.future_time, last_change_time);

    helper.app.next_block(50_000);

    let amp_gamma = helper.query_amp_gamma().unwrap();
    assert_eq!(dec_to_f64(amp_gamma.amp), 42f64);
    assert_eq!(dec_to_f64(amp_gamma.gamma), 0.0000945);
    assert_eq!(amp_gamma.future_time, last_change_time);
}

#[test]
fn check_prices() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("USDC")];

    let helper = Helper::new(&owner, test_coins.clone(), common_pcl_params(), true).unwrap();
    let err = helper.query_prices().unwrap_err();
    assert_eq!(StdError::generic_err("Querier contract error: Generic error: Not implemented.Use { \"observe\" : { \"seconds_ago\" : ... } } instead.")
               , err);
}

#[test]
fn update_owner() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("USDC")];

    let mut helper = Helper::new(&owner, test_coins, common_pcl_params(), true).unwrap();

    let new_owner = String::from("new_owner");

    // New owner
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.clone(),
        expires_in: 100, // seconds
    };

    // Unauthorized check
    let err = helper
        .app
        .execute_contract(
            Addr::unchecked("not_owner"),
            helper.pair_addr.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let err = helper
        .app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            helper.pair_addr.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner
    helper
        .app
        .execute_contract(
            Addr::unchecked(&helper.owner),
            helper.pair_addr.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Claim from invalid addr
    let err = helper
        .app
        .execute_contract(
            Addr::unchecked("invalid_addr"),
            helper.pair_addr.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim ownership
    helper
        .app
        .execute_contract(
            Addr::unchecked(new_owner.clone()),
            helper.pair_addr.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap();

    let config = helper.query_config().unwrap();
    assert_eq!(config.owner.unwrap().to_string(), new_owner)
}

#[test]
fn check_orderbook_integration() {
    let owner = generate_inj_address();
    let test_coins = vec![TestCoin::native("inj"), TestCoin::native("grid")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(0.5),
        ..common_pcl_params()
    };

    let app = mock_inj_app(|_, _, _| {});
    let err = Helper::new_with_app(app, &owner, test_coins.clone(), params.clone(), true, None)
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Market 0x3000cec95658b1a59d143d3829b1bcfce83a06d302a31ab3f1a52bfbd7e5395e was not found"
    );

    let mut app = mock_inj_app(|_, _, _| {});
    app.create_market("inj", "grid").unwrap();
    let mut helper =
        Helper::new_with_app(app, &owner, test_coins.clone(), params, true, None).unwrap();

    // Enabling contract with empty inj balance
    helper
        .app
        .enable_contract(helper.pair_addr.clone(), MockFundingMode::SelfFunded)
        .unwrap();

    let err = helper
        .next_block(false)
        .unwrap_err()
        .root_cause()
        .to_string();
    assert!(err.contains("failed to pay gas fees on begin blocker"));

    // Enabling contract with GrantOnly funding mode
    helper
        .app
        .enable_contract(
            helper.pair_addr.clone(),
            MockFundingMode::GrantOnly(helper.owner.clone()),
        )
        .unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance((500_000f64 * 1e18) as u128),
        helper.assets[&test_coins[1]].with_balance((1_000_000f64 * 1e6) as u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    for _ in 0..50 {
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[1]].with_balance((1000.0 * 1e6) as u128),
                None,
            )
            .unwrap();
        helper.next_block(false).unwrap();
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[0]].with_balance((500.0 * 1e18) as u128),
                None,
            )
            .unwrap();
        helper.next_block(false).unwrap();
    }

    let err = helper
        .try_update_ticks(&Addr::unchecked("random_user"))
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ticks are already up to date"
    );

    let ob_state = helper.query_ob_config_smart().unwrap();
    assert_eq!(ob_state.orders_number, 5);
    assert_eq!(ob_state.need_reconcile, false); // sudo endpoint was already executed and liq. deployed in OB
    assert_eq!(ob_state.ready, true);

    let ob_config = helper.query_ob_config().unwrap();
    let querier_wrapper = helper.app.wrap();
    let inj_querier = InjectiveQuerier::new(&querier_wrapper);

    let inj_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"inj".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let grid_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"grid".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    assert_eq!(inj_deposit, 2489_981000000000000000);
    assert_eq!(grid_deposit, 4979_051501);

    let inj_pool = helper.coin_balance(&test_coins[0], &helper.pair_addr);
    let grid_pool = helper.coin_balance(&test_coins[1], &helper.pair_addr);

    assert_eq!(inj_pool, 497542_933893233248565365);
    assert_eq!(grid_pool, 995085_325039);

    // total liquidity is close to initial provided liquidity
    let total_inj = inj_deposit + inj_pool;
    let total_grid = grid_deposit + grid_pool;
    assert_eq!(total_inj, 500032_914893233248565365);
    assert_eq!(total_grid, 100006_4376540);

    let random_user = generate_inj_address();

    // Try to withdraw liquidity from orderbook while contract is still active
    let err = helper
        .app
        .execute_contract(
            random_user,
            helper.pair_addr.clone(),
            &ExecuteMsg::WithdrawFromOrderbook {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Failed to withdraw liquidity from orderbook: contract is active"
    );

    // Deactivate contract (on chain level). This should trigger withdraw from orderbook
    helper
        .app
        .deactivate_contract(helper.pair_addr.clone())
        .unwrap();

    let querier_wrapper = helper.app.wrap();
    let inj_querier = InjectiveQuerier::new(&querier_wrapper);
    let inj_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"inj".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let grid_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"grid".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    assert_eq!(inj_deposit, 0);
    assert_eq!(grid_deposit, 0);

    let inj_pool = helper.coin_balance(&test_coins[0], &helper.pair_addr);
    let grid_pool = helper.coin_balance(&test_coins[1], &helper.pair_addr);

    assert_eq!(inj_pool, total_inj);
    assert_eq!(grid_pool, total_grid);
}

#[test]
fn check_last_withdraw() {
    let owner = generate_inj_address();

    let test_coins = vec![TestCoin::native("inj"), TestCoin::native("grid")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(0.5),
        ..common_pcl_params()
    };
    let mut helper = Helper::new(&owner, test_coins.clone(), params, true).unwrap();

    // Enabling contract with empty inj balance
    helper
        .app
        .enable_contract(
            helper.pair_addr.clone(),
            MockFundingMode::GrantOnly(helper.owner.clone()),
        )
        .unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance((500_000f64 * 1e18) as u128),
        helper.assets[&test_coins[1]].with_balance((1_000_000f64 * 1e6) as u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    for _ in 0..10 {
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[1]].with_balance((1000.0 * 1e6) as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[0]].with_balance((500.0 * 1e18) as u128),
                None,
            )
            .unwrap();
    }

    // Owner is a major LP and his liquidity intersects with liquidity deployed in orderbook.
    // Check that in that case all orders are cancelled and liquidity withdrawn.
    let owner_lp_balance = helper.token_balance(&helper.lp_token, &owner);
    helper
        .withdraw_liquidity(&owner, owner_lp_balance, vec![])
        .unwrap();
    let contract_balances = helper.query_pool().unwrap();
    // small dust left in balance because it was initial provide fee + a tiny compensation for rounding errors
    assert_eq!(contract_balances.assets[0].amount.u128(), 707823207120245);
    assert_eq!(contract_balances.assets[1].amount.u128(), 1416);
}

#[test]
fn check_deactivate_orderbook() {
    let owner = generate_inj_address();

    let test_coins = vec![TestCoin::native("inj"), TestCoin::native("grid")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(0.5),
        ..common_pcl_params()
    };
    let mut helper = Helper::new(&owner, test_coins.clone(), params, true).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance((500_000f64 * 1e18) as u128),
        helper.assets[&test_coins[1]].with_balance((1_000_000f64 * 1e6) as u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper
        .app
        .enable_contract(
            helper.pair_addr.clone(),
            MockFundingMode::GrantOnly(helper.owner.clone()),
        )
        .unwrap();

    let ob_config = helper.query_ob_config().unwrap();

    for _ in 0..50 {
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[1]].with_balance((1000.0 * 1e6) as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[0]].with_balance((500.0 * 1e18) as u128),
                None,
            )
            .unwrap();
    }

    let querier_wrapper = helper.app.wrap();
    let inj_querier = InjectiveQuerier::new(&querier_wrapper);

    let inj_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"inj".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let grid_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"grid".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let inj_pool = helper.coin_balance(&test_coins[0], &helper.pair_addr);
    let grid_pool = helper.coin_balance(&test_coins[1], &helper.pair_addr);

    // total liquidity is close to initial provided liquidity
    let mut total_inj = inj_deposit + inj_pool;
    let mut total_grid = grid_deposit + grid_pool;
    assert_eq!(total_inj, 500032_914893233248565365);
    assert_eq!(total_grid, 100006_4376540);

    let random_user = generate_inj_address();

    // Try to withdraw liquidity from orderbook while contract is still active
    let err = helper
        .app
        .execute_contract(
            random_user.clone(),
            helper.pair_addr.clone(),
            &ExecuteMsg::WithdrawFromOrderbook {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Failed to withdraw liquidity from orderbook: contract is active"
    );

    // Simulating the case when contract was not able to deactivate itself (for example, because of out of gas)
    helper.app.init_modules(|router, _, _| {
        router
            .custom
            .enabled_contracts
            .borrow_mut()
            .entry(helper.pair_addr.clone())
            .and_modify(|(_, enabled)| *enabled = false);
    });

    // Check that liquidity is still sits on orderbook
    let inj_pool = helper.coin_balance(&test_coins[0], &helper.pair_addr);
    assert_ne!(inj_pool, 0);

    // Simulate trade after contract was kicked out
    let grid_amnt = 9.9e6 as u128;
    let maker_fee = 12870_u128;
    let module_addr = helper.app.init_modules(|router, _, _| {
        let mut deposits = router.custom.deposit.borrow_mut();
        let subacc = deposits
            .get_mut(&ob_config.subaccount.clone().into())
            .unwrap();
        subacc[0].amount += Uint128::from(5e18 as u128);
        subacc[1].amount -= Uint128::from(grid_amnt);

        router.custom.module_addr.clone()
    });
    helper
        .app
        .send_tokens(
            helper.owner.clone(),
            module_addr.clone(),
            &coins(5e18 as u128, "inj"),
        )
        .unwrap();
    helper
        .app
        .send_tokens(
            module_addr,
            helper.owner.clone(),
            &coins(grid_amnt, "grid"),
        )
        .unwrap();
    total_inj += 5e18 as u128;
    total_grid -= grid_amnt + maker_fee; // + maker fee

    helper.next_block(true).unwrap();

    let maker_bal_before = helper.coin_balance(&test_coins[1], &helper.maker);
    let oracle_price_before = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .oracle_price;

    // Withdraw should succeed
    helper
        .app
        .execute_contract(
            random_user.clone(),
            helper.pair_addr.clone(),
            &ExecuteMsg::WithdrawFromOrderbook {},
            &[],
        )
        .unwrap();

    let maker_bal_after = helper.coin_balance(&test_coins[1], &helper.maker);
    assert_eq!(maker_bal_after - maker_bal_before, maker_fee);
    let oracle_price_after = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .oracle_price;
    // Check that oracle price was updated
    assert_ne!(oracle_price_after, oracle_price_before);

    let querier_wrapper = helper.app.wrap();
    let inj_querier = InjectiveQuerier::new(&querier_wrapper);
    let inj_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"inj".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let grid_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"grid".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    assert_eq!(inj_deposit, 0);
    assert_eq!(grid_deposit, 0);

    let inj_pool = helper.coin_balance(&test_coins[0], &helper.pair_addr);
    let grid_pool = helper.coin_balance(&test_coins[1], &helper.pair_addr);

    assert_eq!(inj_pool, total_inj);
    assert_eq!(grid_pool, total_grid);

    // Withdraw endpoint is not blocked but it does nothing
    helper
        .app
        .execute_contract(
            random_user,
            helper.pair_addr.clone(),
            &ExecuteMsg::WithdrawFromOrderbook {},
            &[],
        )
        .unwrap();

    let inj_pool = helper.coin_balance(&test_coins[0], &helper.pair_addr);
    let grid_pool = helper.coin_balance(&test_coins[1], &helper.pair_addr);

    assert_eq!(inj_pool, total_inj);
    assert_eq!(grid_pool, total_grid);

    // Next swap works as usual
    helper
        .swap(
            &owner,
            &helper.assets[&test_coins[0]].with_balance(1e18 as u128),
            None,
        )
        .unwrap();

    // Enable contract again. Checking that FIRST swap after failed deactivation will run CL logic and send maker fees
    helper
        .app
        .enable_contract(
            helper.pair_addr.clone(),
            MockFundingMode::GrantOnly(helper.owner.clone()),
        )
        .unwrap();
    helper
        .swap(
            &owner,
            &helper.assets[&test_coins[1]].with_balance((1000.0 * 1e6) as u128),
            None,
        )
        .unwrap();
    helper.next_block(true).unwrap();

    // Simulating the case when contract was not able to deactivate itself (for example, because of out of gas)
    helper.app.init_modules(|router, _, _| {
        router
            .custom
            .enabled_contracts
            .borrow_mut()
            .entry(helper.pair_addr.clone())
            .and_modify(|(_, enabled)| *enabled = false);
    });

    // Simulate trade after contract was kicked out
    let module_addr = helper.app.init_modules(|router, _, _| {
        let mut deposits = router.custom.deposit.borrow_mut();
        let subacc = deposits
            .get_mut(&ob_config.subaccount.clone().into())
            .unwrap();
        subacc[0].amount += Uint128::from(5e18 as u128);
        subacc[1].amount -= Uint128::from(9.9e6 as u128);

        router.custom.module_addr.clone()
    });
    helper
        .app
        .send_tokens(
            helper.owner.clone(),
            module_addr.clone(),
            &coins(5e18 as u128, "inj"),
        )
        .unwrap();
    helper
        .app
        .send_tokens(
            module_addr,
            helper.owner.clone(),
            &coins(9.9e6 as u128, "grid"),
        )
        .unwrap();

    helper.next_block(true).unwrap();

    let maker_bal_before = helper.coin_balance(&test_coins[1], &helper.maker);
    let oracle_price_before = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .oracle_price;

    // Usual swap triggers CL logic and sends maker fees
    helper
        .swap(
            &owner,
            &helper.assets[&test_coins[1]].with_balance(1000e6 as u128),
            None,
        )
        .unwrap();

    let maker_bal_after = helper.coin_balance(&test_coins[1], &helper.maker);
    assert_eq!(maker_bal_after - maker_bal_before, 12908);
    let oracle_price_after = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .oracle_price;
    // Check that oracle price was updated
    assert_ne!(oracle_price_after, oracle_price_before);

    // Next swaps work as usual
    helper
        .swap(
            &owner,
            &helper.assets[&test_coins[0]].with_balance(1e18 as u128),
            None,
        )
        .unwrap();

    // Enable contract again. Checking that FIRST provide after failed deactivation will run CL logic and send maker fees
    helper
        .app
        .enable_contract(
            helper.pair_addr.clone(),
            MockFundingMode::GrantOnly(helper.owner.clone()),
        )
        .unwrap();
    helper
        .swap(
            &owner,
            &helper.assets[&test_coins[0]].with_balance(1e18 as u128),
            None,
        )
        .unwrap();
    helper.next_block(true).unwrap();

    // Simulating the case when contract was not able to deactivate itself (for example, because of out of gas)
    helper.app.init_modules(|router, _, _| {
        router
            .custom
            .enabled_contracts
            .borrow_mut()
            .entry(helper.pair_addr.clone())
            .and_modify(|(_, enabled)| *enabled = false);
    });

    // Simulate trade after contract was kicked out
    let module_addr = helper.app.init_modules(|router, _, _| {
        let mut deposits = router.custom.deposit.borrow_mut();
        let subacc = deposits.get_mut(&ob_config.subaccount.into()).unwrap();
        subacc[0].amount += Uint128::from(5e18 as u128);
        subacc[1].amount -= Uint128::from(9.9e6 as u128);

        router.custom.module_addr.clone()
    });
    helper
        .app
        .send_tokens(
            helper.owner.clone(),
            module_addr.clone(),
            &coins(5e18 as u128, "inj"),
        )
        .unwrap();
    helper
        .app
        .send_tokens(
            module_addr,
            helper.owner.clone(),
            &coins(9.9e6 as u128, "grid"),
        )
        .unwrap();

    helper.next_block(true).unwrap();

    let maker_bal_before = helper.coin_balance(&test_coins[1], &helper.maker);
    let oracle_price_before = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .oracle_price;

    // Usual provide triggers CL logic and sends maker fees
    helper
        .provide_liquidity(
            &owner,
            &[helper.assets[&test_coins[1]].with_balance((1.0 * 1e6) as u128)],
        )
        .unwrap();

    let maker_bal_after = helper.coin_balance(&test_coins[1], &helper.maker);
    assert_eq!(maker_bal_after - maker_bal_before, 13024);
    let oracle_price_after = helper
        .query_config()
        .unwrap()
        .pool_state
        .price_state
        .oracle_price;
    // Check that oracle price was updated
    assert_ne!(oracle_price_after, oracle_price_before);
}

#[test]
fn test_migrate_cl_to_orderbook_cl() {
    let owner = generate_inj_address();

    let test_coins = vec![TestCoin::native("inj"), TestCoin::native("grid")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(0.5),
        ..common_pcl_params()
    };
    let mut app = mock_inj_app(|_, _, _| {});
    let market_id = app
        .create_market(
            &test_coins[0].denom().unwrap(),
            &test_coins[1].denom().unwrap(),
        )
        .unwrap();
    let mut helper =
        Helper::new_with_app(app, &owner, test_coins.clone(), params.clone(), false, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(500_000e18 as u128),
        helper.assets[&test_coins[1]].with_balance(1_000_000e6 as u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Make some swaps
    for _ in 0..10 {
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[1]].with_balance(1000e6 as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[0]].with_balance(500e18 as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
    }

    let migrate_msg = MigrateMsg::MigrateToOrderbook {
        params: OrderbookConfig {
            market_id: "0x3001cec95658b1a59d143d3829b1bcfce83a06d302a31ab3f1a52bfbd7e5395e"
                .to_string(),
            orders_number: 5,
            min_trades_to_avg: 1,
        },
    };

    let new_code_id = helper.app.store_code(orderbook_pair_contract());
    let err = helper
        .app
        .migrate_contract(
            owner.clone(),
            helper.pair_addr.clone(),
            &migrate_msg,
            new_code_id,
        )
        .unwrap_err();

    let err_msg = err.root_cause().to_string();
    assert!(
        err_msg.contains("Invalid market id"),
        "Wrong error message {}",
        err_msg
    );

    let migrate_msg = MigrateMsg::MigrateToOrderbook {
        params: OrderbookConfig {
            market_id,
            orders_number: 5,
            min_trades_to_avg: 1,
        },
    };
    helper
        .app
        .migrate_contract(
            owner.clone(),
            helper.pair_addr.clone(),
            &migrate_msg,
            new_code_id,
        )
        .unwrap();

    let config = helper.query_config().unwrap();
    assert_eq!(
        config.pair_info.pair_type,
        PairType::Custom("concentrated_inj_orderbook".to_string())
    );
    assert_eq!(config.pool_state.price_state.price_scale.to_string(), "0.5");

    helper
        .app
        .enable_contract(
            helper.pair_addr.clone(),
            MockFundingMode::GrantOnly(helper.owner.clone()),
        )
        .unwrap();

    for _ in 0..50 {
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[1]].with_balance(1000e6 as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[0]].with_balance(500e18 as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
    }

    // Check that orders have been created
    let querier_wrapper = helper.app.wrap();
    let inj_querier = InjectiveQuerier::new(&querier_wrapper);
    let ob_config = helper.query_ob_config().unwrap();

    let inj_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"inj".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let grid_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"grid".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    assert_eq!(inj_deposit, 2489_976000000000000000);
    assert_eq!(grid_deposit, 4979_061419);

    let inj_pool = helper.coin_balance(&test_coins[0], &helper.pair_addr);
    let grid_pool = helper.coin_balance(&test_coins[1], &helper.pair_addr);

    // Total liquidity is close to initial provided liquidity
    let total_inj = inj_deposit + inj_pool;
    let total_grid = grid_deposit + grid_pool;
    assert_eq!(total_inj, 500039_497794763370151009);
    assert_eq!(total_grid, 100007_7251964);
}

#[test]
fn test_wrong_assets_order() {
    let owner = generate_inj_address();

    let test_coins = vec![TestCoin::native("inj"), TestCoin::native("grid")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(0.5),
        ..common_pcl_params()
    };
    let mut app = mock_inj_app(|_, _, _| {});
    let market_id = app
        .create_market(
            // NOTE: we intentionally changed denoms order
            &test_coins[1].denom().unwrap(),
            &test_coins[0].denom().unwrap(),
        )
        .unwrap();
    let mut helper =
        Helper::new_with_app(app, &owner, test_coins.clone(), params.clone(), false, None).unwrap();

    let migrate_msg = MigrateMsg::MigrateToOrderbook {
        params: OrderbookConfig {
            market_id,
            orders_number: 5,
            min_trades_to_avg: *MIN_TRADES_TO_AVG_LIMITS.start(),
        },
    };
    let new_code_id = helper.app.store_code(orderbook_pair_contract());
    let err = helper
        .app
        .migrate_contract(
            owner.clone(),
            helper.pair_addr.clone(),
            &migrate_msg,
            new_code_id,
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Pair asset infos have different order than market: inj-grid while market has grid-inj");

    let mut app = mock_inj_app(|_, _, _| {});
    let market_id = app
        .create_market(
            // NOTE: we intentionally changed denoms order
            &test_coins[1].denom().unwrap(),
            &test_coins[0].denom().unwrap(),
        )
        .unwrap();
    let err = Helper::new_with_app(
        app,
        &owner,
        test_coins.clone(),
        params.clone(),
        true,
        Some(market_id),
    )
    .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Pair asset infos have different order than market: inj-grid while market has grid-inj");
}

#[test]
fn test_feegrant_mode() {
    let owner = generate_inj_address();

    let test_coins = vec![TestCoin::native("inj"), TestCoin::native("grid")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(0.5),
        ..common_pcl_params()
    };
    let mut app = mock_inj_app(|_, _, _| {});
    app.create_market(
        &test_coins[0].denom().unwrap(),
        &test_coins[1].denom().unwrap(),
    )
    .unwrap();
    let mut helper =
        Helper::new_with_app(app, &owner, test_coins.clone(), params.clone(), true, None).unwrap();

    helper
        .app
        .enable_contract(helper.pair_addr.clone(), MockFundingMode::SelfFunded)
        .unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(500_000e18 as u128),
        helper.assets[&test_coins[1]].with_balance(1_000_000e6 as u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // BB should be disabled since pool has inj and SelfFunded mode.
    let ob_config = helper.query_ob_config().unwrap();
    assert!(!ob_config.enabled);

    // Make some swaps
    for _ in 0..10 {
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[1]].with_balance(1000e6 as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[0]].with_balance(500e18 as u128),
                None,
            )
            .unwrap();
    }

    // After swaps contract must not deploy any liquidity because it is disabled
    let querier_wrapper = helper.app.wrap();
    let inj_querier = InjectiveQuerier::new(&querier_wrapper);
    let inj_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"inj".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let grid_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"grid".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    assert_eq!(inj_deposit, 0);
    assert_eq!(grid_deposit, 0);

    // Register contract with proper funding mode
    helper
        .app
        .enable_contract(
            helper.pair_addr.clone(),
            MockFundingMode::GrantOnly(helper.owner.clone()),
        )
        .unwrap();

    // Make some swaps
    for _ in 0..10 {
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[1]].with_balance(1000e6 as u128),
                None,
            )
            .unwrap();
        helper.next_block(true).unwrap();
        helper
            .swap(
                &owner,
                &helper.assets[&test_coins[0]].with_balance(500e18 as u128),
                None,
            )
            .unwrap();
    }

    // Contract must start deploying its liquidity in the order book
    let querier_wrapper = helper.app.wrap();
    let inj_querier = InjectiveQuerier::new(&querier_wrapper);
    let inj_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"inj".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    let grid_deposit: u128 = inj_querier
        .query_subaccount_deposit(&ob_config.subaccount, &"grid".to_string())
        .unwrap()
        .deposits
        .total_balance
        .into();
    assert!(inj_deposit > 0, "subaccount inj deposit is zero");
    assert!(grid_deposit > 0, "subaccount grid deposit is zero");
}

#[test]
fn provide_liquidity_with_autostaking_to_generator() {
    use gridiron_mocks::{
        gridiron_address, MockConcentratedPairInjBuilder, MockGeneratorBuilder,
    };
    let gridiron = gridiron_address();

    let app = Rc::new(RefCell::new(mock_inj_app(|router, _, storage| {
        router
            .bank
            .init_balance(
                storage,
                &gridiron,
                vec![
                    Coin {
                        denom: "ucosmos".to_owned(),
                        amount: Uint128::new(1_000_000_000000),
                    },
                    Coin {
                        denom: "ustake".to_owned(),
                        amount: Uint128::new(1_000_000_000000),
                    },
                ],
            )
            .unwrap();
    })));
    app.borrow_mut().create_market("ucosmos", "ustake").unwrap();

    let generator = MockGeneratorBuilder::new(&app).instantiate();

    let factory = generator.factory();

    let ucosmos = native_asset_info("ucosmos".to_owned());
    let ustake = native_asset_info("ustake".to_owned());

    let pair = MockConcentratedPairInjBuilder::new(&app)
        .with_factory(&factory)
        .with_asset(&ucosmos)
        .with_asset(&ustake)
        .instantiate(None);

    pair.mint_allow_provide_and_stake(
        &gridiron,
        &[
            ucosmos.with_balance(1_000_000000u128),
            ustake.with_balance(1_000_000000u128),
        ],
    );

    assert_eq!(pair.lp_token().balance(&pair.address), Uint128::new(1000));
    assert_eq!(
        generator.query_deposit(&pair.lp_token(), &gridiron),
        Uint128::new(999_999000),
    );
}

#[test]
fn provide_withdraw_provide() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("uluna")];

    let params = ConcentratedPoolParams {
        amp: f64_to_dec(10f64),
        price_scale: Decimal::from_ratio(10u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params, true).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(10_938039u128),
        helper.assets[&test_coins[1]].with_balance(1_093804u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    helper.app.next_block(90);
    helper.provide_liquidity(&owner, &assets).unwrap();

    helper.app.next_block(90);
    let uusd = helper.assets[&test_coins[0]].with_balance(5_000000u128);
    helper.swap(&owner, &uusd, Some(f64_to_dec(0.5))).unwrap();

    helper.app.next_block(600);
    // Withdraw all
    let lp_amount = helper.token_balance(&helper.lp_token, &owner);
    helper
        .withdraw_liquidity(&owner, lp_amount, vec![])
        .unwrap();

    // Provide again
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.5)))
        .unwrap();
}

#[test]
fn provide_withdraw_slippage() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("uluna")];

    let params = ConcentratedPoolParams {
        amp: f64_to_dec(10f64),
        price_scale: Decimal::from_ratio(10u8, 1u8),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params, true).unwrap();

    // Fully balanced provide
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(10_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000000u128),
    ];
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.02)))
        .unwrap();

    // Imbalanced provide. Slippage is more than 2% while we enforce 2% max slippage
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(5_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_000000u128),
    ];
    let err = helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.02)))
        .unwrap_err();
    assert_eq!(
        ContractError::PclError(PclError::MaxSpreadAssertion {}),
        err.downcast().unwrap()
    );
    // With 3% slippage it should work
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.03)))
        .unwrap();

    // Provide with a huge imbalance. Slippage is ~42.2%
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(1000_000000u128),
        helper.assets[&test_coins[1]].with_balance(1000_000000u128),
    ];
    let err = helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.02)))
        .unwrap_err();
    assert_eq!(
        ContractError::PclError(PclError::MaxSpreadAssertion {}),
        err.downcast().unwrap(),
    );
    helper
        .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.5)))
        .unwrap();
}

#[test]
fn check_small_trades() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::native("uluna")];

    let params = ConcentratedPoolParams {
        price_scale: f64_to_dec(4.360000915600192),
        ..common_pcl_params()
    };

    let mut helper = Helper::new(&owner, test_coins.clone(), params, true).unwrap();

    // Fully balanced but small provide
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(8_000000u128),
        helper.assets[&test_coins[1]].with_balance(1_834862u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    // Trying to mess the last price with lowest possible swap
    for _ in 0..1000 {
        helper.app.next_block(30);
        let offer_asset = helper.assets[&test_coins[1]].with_balance(1u8);
        helper
            .swap_full_params(&owner, &offer_asset, None, Some(Decimal::MAX))
            .unwrap();
    }

    // Check that after price scale adjustments (even they are small) internal value is still nearly balanced
    let config = helper.query_config().unwrap();
    let pool = helper
        .query_pool()
        .unwrap()
        .assets
        .into_iter()
        .map(|asset| asset.amount.to_decimal256(6u8).unwrap())
        .collect_vec();

    let ixs = [pool[0], pool[1] * config.pool_state.price_state.price_scale];
    let relative_diff = ixs[0].abs_diff(ixs[1]) / max(&ixs).unwrap();

    assert!(
        relative_diff < Decimal256::percent(3),
        "Internal PCL value is off. Relative_diff: {}",
        relative_diff
    );

    // Trying to mess the last price with lowest possible provide
    for _ in 0..1000 {
        helper.app.next_block(30);
        let assets = vec![helper.assets[&test_coins[1]].with_balance(1u8)];
        helper
            .provide_liquidity_with_slip_tolerance(&owner, &assets, Some(f64_to_dec(0.5)))
            .unwrap();
    }

    // Check that after price scale adjustments (even they are small) internal value is still nearly balanced
    let config = helper.query_config().unwrap();
    let pool = helper
        .query_pool()
        .unwrap()
        .assets
        .into_iter()
        .map(|asset| asset.amount.to_decimal256(6u8).unwrap())
        .collect_vec();

    let ixs = [pool[0], pool[1] * config.pool_state.price_state.price_scale];
    let relative_diff = ixs[0].abs_diff(ixs[1]) / max(&ixs).unwrap();

    assert!(
        relative_diff < Decimal256::percent(3),
        "Internal PCL value is off. Relative_diff: {}",
        relative_diff
    );
}
