use optionstratlib::model::types::ExpirationDate;
use optionstratlib::model::types::PositiveF64;
use optionstratlib::pos;
use optionstratlib::strategies::base::Strategies;
use optionstratlib::strategies::bear_call_spread::BearCallSpread;
use optionstratlib::utils::logger::setup_logger;
use optionstratlib::visualization::utils::Graph;
use std::error::Error;

#[test]
fn test_bear_call_spread_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();
    // Define inputs for the BearCallSpread strategy
    let underlying_price = pos!(5781.88);

    let strategy = BearCallSpread::new(
        "SP500".to_string(),
        underlying_price, // underlying_price
        pos!(5750.0),     // long_strike_itm
        pos!(5820.0),     // short_strike
        ExpirationDate::Days(2.0),
        0.18,      // implied_volatility
        0.05,      // risk_free_rate
        0.0,       // dividend_yield
        pos!(2.0), // long quantity
        85.04,     // premium_long
        29.85,     // premium_short
        0.78,      // open_fee_long
        0.78,      // open_fee_long
        0.73,      // close_fee_long
        0.73,      // close_fee_short
    );

    // Assertions to validate strategy properties and computations
    assert_eq!(strategy.title(), "Bear Call Spread Strategy:\n\tUnderlying: SP500 @ $5750 Short Call European Option\n\tUnderlying: SP500 @ $5820 Long Call European Option");
    assert_eq!(strategy.get_break_even_points().len(), 1);
    assert_eq!(strategy.net_premium_received(), 104.34);
    assert!(strategy.max_profit().is_ok());
    assert!(strategy.max_loss().is_ok());
    assert_eq!(strategy.max_profit()?, pos!(104.34));
    assert_eq!(strategy.max_loss()?, pos!(35.66));
    assert_eq!(strategy.total_cost(), pos!(229.58));
    assert_eq!(strategy.fees(), 3.02);
    assert!(strategy.profit_area() > 0.0);
    assert!(strategy.profit_ratio() > 0.0);

    Ok(())
}