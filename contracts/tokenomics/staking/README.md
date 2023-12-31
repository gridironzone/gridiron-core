# Gridiron xGRID Staking

This staking contract allows GRID holders to stake their tokens in exchange for xGRID. The amount of GRID they can claim later increases as accrued fees in the Maker contract get swapped to GRID which is then sent to stakers.

---

## InstantiateMsg

Initializes the contract with the token code ID used by GRID and the GRID token address.

```json
{
  "token_code_id": 123,
  "deposit_token_addr": "terra..."
}
```

## ExecuteMsg

### `receive`

CW20 receive msg.

```json
{
  "receive": {
    "sender": "terra...",
    "amount": "123",
    "msg": "<base64_encoded_json_string>"
  }
}
```

#### `Enter`

Deposits GRID in the xGRID staking contract.

Execute this message by calling the GRID token contract and use a message like this:
```json
{
  "send": {
    "contract": <StakingContractAddress>,
    "amount": "999",
    "msg": "base64-encodedStringOfWithdrawMsg"
  }
}
```

In `send.msg`, you may encode this JSON string into base64 encoding:
```json
{
  "enter": {}
}
```

#### `leave`

Burns xGRID and unstakes underlying GRID (initial staked amount + accrued GRID since staking).

Execute this message by calling the xGRID token contract and use a message like this:
```json
{
  "send": {
    "contract": <StakingContractAddress>,
    "amount": "999",
    "msg": "base64-encodedStringOfWithdrawMsg"
  }
}
```

In `send.msg` you may encode this JSON string into base64 encoding:
```json
{
  "leave": {}
}
```

## QueryMsg

All query messages are described below. A custom struct is defined for each query response.

### `config`

Returns the GRID and xGRID addresses.

```json
{
  "config": {}
}
```

### `get_total_shares`

Returns the total amount of xGRID tokens.

```json
{
  "get_total_shares": {}
}
```

### `get_total_deposit`

Returns the total amount of GRID deposits in the staking contract.

```json
{
  "get_total_deposit": {}
}
```
