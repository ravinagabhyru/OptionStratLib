use approx::assert_relative_eq;
use optionstratlib::model::types::ExpirationDate;
use optionstratlib::model::types::{PositiveF64, PZERO};
use optionstratlib::strategies::base::Strategies;
use optionstratlib::strategies::bull_call_spread::BullCallSpread;
use optionstratlib::utils::logger::setup_logger;
use optionstratlib::visualization::utils::Graph;
use optionstratlib::{assert_positivef64_relative_eq, pos};
use std::error::Error;

#[test]
fn test_bull_call_spread_basic_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    let strategy = BullCallSpread::new(
        "GOLD".to_string(),
        pos!(2505.8), // underlying_price
        pos!(2460.0), // long_strike_itm
        pos!(2515.0), // short_strike
        ExpirationDate::Days(30.0),
        0.2,       // implied_volatility
        0.05,      // risk_free_rate
        0.0,       // dividend_yield
        pos!(1.0), // quantity
        27.26,     // premium_long
        5.33,      // premium_short
        0.58,      // open_fee_long
        0.58,      // close_fee_long
        0.55,      // close_fee_short
        0.54,      // open_fee_short
    );

    // Validate strategy properties
    assert_eq!(strategy.title(), "Bull Call Spread Strategy:\n\tUnderlying: GOLD @ $2460 Long Call European Option\n\tUnderlying: GOLD @ $2515 Short Call European Option");
    assert_eq!(strategy.get_break_even_points().len(), 1);

    // Validate financial calculations
    assert_relative_eq!(strategy.net_premium_received(), -24.18, epsilon = 0.001);
    assert!(strategy.max_profit().is_ok());
    assert!(strategy.max_loss().is_ok());
    assert_positivef64_relative_eq!(strategy.max_profit()?, pos!(30.82), pos!(0.0001));
    assert_positivef64_relative_eq!(strategy.max_loss()?, pos!(24.18), pos!(0.0001));
    assert_positivef64_relative_eq!(strategy.total_cost(), pos!(32.66), pos!(0.0001));
    assert_eq!(strategy.fees(), 2.25);

    // Test price range calculations
    let test_price_range: Vec<PositiveF64> = (2400..2600)
        .map(|x| PositiveF64::new(x as f64).unwrap())
        .collect();
    assert!(!test_price_range.is_empty());
    assert_eq!(test_price_range.len(), 200);

    // Validate strike prices relationship
    assert!(
        pos!(2460.0) < pos!(2515.0),
        "Long strike should be less than short strike in a bull call spread"
    );

    // Validate break-even point
    let break_even = strategy.break_even();
    assert!(
        break_even[0] > pos!(2460.0),
        "Break-even should be between strikes"
    );

    Ok(())
}
