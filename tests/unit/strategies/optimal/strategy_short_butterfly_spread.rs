use approx::assert_relative_eq;
use num_traits::ToPrimitive;
use optionstratlib::chains::chain::OptionChain;
use optionstratlib::{pos, Positive};
use optionstratlib::strategies::base::{Optimizable, Strategies};
use optionstratlib::strategies::butterfly_spread::ShortButterflySpread;
use optionstratlib::strategies::utils::FindOptimalSide;
use optionstratlib::utils::setup_logger;
use optionstratlib::ExpirationDate;
use std::error::Error;
use rust_decimal_macros::dec;

#[test]
fn test_short_butterfly_spread_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    // Define inputs for the ShortButterflySpread strategy
    let underlying_price = pos!(5781.88);

    let mut strategy = ShortButterflySpread::new(
        "SP500".to_string(),
        underlying_price,   // underlying_price
        pos!(5700.0),   // short_strike_itm
        pos!(5780.0),   // long_strike
        pos!(5850.0),   // short_strike_otm
        ExpirationDate::Days(2.0),
        pos!(0.18),   // implied_volatility
        dec!(0.05),   // risk_free_rate
        Positive::ZERO,   // dividend_yield
        pos!(3.0),   // long quantity
        119.01,   // premium_long
        66.0,   // premium_short
        29.85,   // open_fee_long
        4.0,   // open_fee_long
    );

    let option_chain =
        OptionChain::load_from_json("./examples/Chains/SP500-18-oct-2024-5781.88.json")?;
    strategy.best_area(&option_chain, FindOptimalSide::All);
    assert_relative_eq!(
        strategy.profit_area().unwrap().to_f64().unwrap(),
        778.4392,
        epsilon = 0.001
    );
    strategy.best_ratio(&option_chain, FindOptimalSide::Upper);
    assert_relative_eq!(
        strategy.profit_ratio().unwrap().to_f64().unwrap(),
        535.8086,
        epsilon = 0.001
    );

    Ok(())
}
