use cosmwasm_schema::write_api;

use gridiron_cw20_ics20::msg::{ExecuteMsg, InitMsg, QueryMsg};

fn main() {
    write_api! {
        instantiate: InitMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
