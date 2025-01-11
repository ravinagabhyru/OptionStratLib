use approx::assert_relative_eq;
use optionstratlib::greeks::equations::Greeks;
use optionstratlib::strategies::delta_neutral::DeltaAdjustment::NoAdjustmentNeeded;
use optionstratlib::strategies::delta_neutral::DeltaNeutrality;
use optionstratlib::strategies::ShortStrangle;
use optionstratlib::utils::setup_logger;
use optionstratlib::{assert_decimal_eq, pos};
use optionstratlib::{ExpirationDate, Positive};
use rust_decimal_macros::dec;
use std::error::Error;

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
fn test_short_strangle_with_greeks_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    // Define inputs for the ShortStrangle strategy
    let underlying_price = pos!(7138.5);

    let strategy = ShortStrangle::new(
        "CL".to_string(),
        underlying_price, // underlying_price
        pos!(7450.0),     // call_strike
        pos!(7050.0),     // put_strike
        ExpirationDate::Days(pos!(45.0)),
        pos!(0.3745),   // implied_volatility
        dec!(0.05),     // risk_free_rate
        Positive::ZERO, // dividend_yield
        pos!(1.0),      // quantity
        pos!(84.2),     // premium_short_call
        pos!(353.2),    // premium_short_put
        pos!(7.01),     // open_fee_short_call
        pos!(7.01),     // close_fee_short_call
        pos!(7.01),     // open_fee_short_put
        pos!(7.01),     // close_fee_short_put
    );

    let greeks = strategy.greeks();
    let epsilon = dec!(0.001);

    assert_decimal_eq!(greeks.delta, dec!(0.0018), epsilon);
    assert_decimal_eq!(greeks.gamma, dec!(0.0008), epsilon);
    assert_decimal_eq!(greeks.theta, dec!(-2942.0709), epsilon);
    assert_decimal_eq!(greeks.vega, dec!(2501.9092), epsilon);
    assert_decimal_eq!(greeks.rho, dec!(-72.0661), epsilon);
    assert_decimal_eq!(greeks.rho_d, dec!(1.6100), epsilon);

    assert_relative_eq!(
        strategy.calculate_net_delta().net_delta,
        0.0018,
        epsilon = 0.001
    );
    assert_relative_eq!(
        strategy.calculate_net_delta().individual_deltas[0],
        -0.4159,
        epsilon = 0.001
    );
    assert_relative_eq!(
        strategy.calculate_net_delta().individual_deltas[1],
        0.4178,
        epsilon = 0.001
    );
    assert!(strategy.is_delta_neutral());
    assert_eq!(strategy.suggest_delta_adjustments().len(), 1);

    assert_eq!(strategy.suggest_delta_adjustments()[0], NoAdjustmentNeeded);

    Ok(())
}
