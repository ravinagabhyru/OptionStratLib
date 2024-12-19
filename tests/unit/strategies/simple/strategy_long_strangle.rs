use approx::assert_relative_eq;
use optionstratlib::constants::ZERO;
use optionstratlib::model::types::ExpirationDate;
use optionstratlib::model::types::{PositiveF64, PZERO};
use optionstratlib::strategies::base::Strategies;
use optionstratlib::strategies::strangle::LongStrangle;
use optionstratlib::utils::logger::setup_logger;
use optionstratlib::visualization::utils::Graph;
use optionstratlib::{assert_positivef64_relative_eq, pos};
use std::error::Error;

#[test]
fn test_long_strangle_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    // Define inputs for the LongStrangle strategy
    let underlying_price = pos!(7138.5);

    let strategy = LongStrangle::new(
        "CL".to_string(),
        underlying_price, // underlying_price
        pos!(7450.0),     // call_strike
        pos!(7050.0),     // put_strike
        ExpirationDate::Days(45.0),
        0.3745,    // implied_volatility
        0.05,      // risk_free_rate
        0.0,       // dividend_yield
        pos!(1.0), // quantity
        84.2,      // premium_short_call
        353.2,     // premium_short_put
        7.0,       // open_fee_short_call
        7.01,      // close_fee_short_call
        7.01,      // open_fee_short_put
        7.01,      // close_fee_short_put
    );

    // Assertions to validate strategy properties and computations
    assert_eq!(strategy.title(), "Long Strangle Strategy: \n\tUnderlying: CL @ $7450 Long Call European Option\n\tUnderlying: CL @ $7050 Long Put European Option");
    assert_eq!(strategy.get_break_even_points().len(), 2);
    assert_relative_eq!(strategy.net_premium_received(), ZERO, epsilon = 0.001);
    assert!(strategy.max_profit().is_ok());
    assert!(strategy.max_loss().is_ok());
    assert_positivef64_relative_eq!(strategy.max_loss()?, pos!(465.4299), pos!(0.0001));
    assert_positivef64_relative_eq!(strategy.total_cost(), pos!(465.4299), pos!(0.0001));
    assert_eq!(strategy.fees(), 28.03);

    // Test range calculations
    let price_range = strategy.best_range_to_show(pos!(1.0)).unwrap();
    assert!(!price_range.is_empty());
    let break_even_points = strategy.get_break_even_points();
    let range = break_even_points[1] - break_even_points[0];
    assert_relative_eq!(
        (range.value() / 2.0) / underlying_price.value() * 100.0,
        9.3217,
        epsilon = 0.001
    );

    assert!(strategy.profit_area() > 0.0);
    assert!(strategy.profit_ratio() > 0.0);

    // Validate price range in relation to break even points
    assert!(price_range[0] < break_even_points[0]);
    assert!(price_range[price_range.len() - 1] > break_even_points[1]);

    // Additional strategy-specific validations
    assert!(
        strategy.get_break_even_points()[0] < strategy.get_break_even_points()[1],
        "Lower break-even point should be less than upper break-even point"
    );

    // Validate strike prices relationship (characteristic of Long Strangle)
    assert!(
        pos!(7050.0) < pos!(7450.0),
        "Put strike should be less than call strike in a strangle"
    );

    Ok(())
}