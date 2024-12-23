use approx::assert_relative_eq;
use optionstratlib::chains::chain::OptionChain;
use optionstratlib::model::types::ExpirationDate;
use optionstratlib::model::types::PositiveF64;
use optionstratlib::pos;
use optionstratlib::strategies::base::{Optimizable, Strategies};
use optionstratlib::strategies::bull_put_spread::BullPutSpread;
use optionstratlib::strategies::utils::FindOptimalSide;
use optionstratlib::utils::logger::setup_logger;
use std::error::Error;

#[test]
fn test_bull_put_spread_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    // Define inputs for the BullPutSpread strategy
    let underlying_price = pos!(5781.88);

    let mut strategy = BullPutSpread::new(
        "SP500".to_string(),
        underlying_price, // underlying_price
        pos!(5750.0),     // long_strike_itm
        pos!(5920.0),     // short_strike
        ExpirationDate::Days(2.0),
        0.18,      // implied_volatility
        0.05,      // risk_free_rate
        0.0,       // dividend_yield
        pos!(2.0), // long quantity
        15.04,     // premium_long
        89.85,     // premium_short
        0.78,      // open_fee_long
        0.78,      // open_fee_long
        0.73,      // close_fee_long
        0.73,      // close_fee_short
    );

    let option_chain =
        OptionChain::load_from_json("./examples/Chains/SP500-18-oct-2024-5781.88.json")?;
    strategy.best_area(&option_chain, FindOptimalSide::All);
    assert_relative_eq!(strategy.profit_area(), 1584.9157, epsilon = 0.001);
    strategy.best_ratio(&option_chain, FindOptimalSide::Upper);
    assert_relative_eq!(strategy.profit_ratio(), 2115.6573, epsilon = 0.001);

    Ok(())
}
