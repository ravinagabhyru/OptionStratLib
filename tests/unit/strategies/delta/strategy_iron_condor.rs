use approx::assert_relative_eq;
use optionstratlib::greeks::Greeks;
use optionstratlib::model::types::{ExpirationDate, OptionStyle};
use optionstratlib::strategies::delta_neutral::DeltaAdjustment::BuyOptions;
use optionstratlib::strategies::delta_neutral::DeltaNeutrality;
use optionstratlib::strategies::iron_condor::IronCondor;
use optionstratlib::utils::setup_logger;
use optionstratlib::{assert_decimal_eq, pos, Positive};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::error::Error;
use std::str::FromStr;

#[test]
fn test_iron_condor_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    // Define inputs for the IronCondor strategy
    let underlying_price = pos!(2646.9);

    let strategy = IronCondor::new(
        "GOLD".to_string(),
        underlying_price, // underlying_price
        pos!(2725.0),     // short_call_strike
        pos!(2560.0),     // short_put_strike
        pos!(2800.0),     // long_call_strike
        pos!(2500.0),     // long_put_strike
        ExpirationDate::Days(pos!(30.0)),
        pos!(0.1548),   // implied_volatility
        dec!(0.05),     // risk_free_rate
        Positive::ZERO, // dividend_yield
        pos!(2.0),      // quantity
        pos!(38.8),     // premium_short_call
        pos!(30.4),     // premium_short_put
        pos!(23.3),     // premium_long_call
        pos!(16.8),     // premium_long_put
        pos!(0.96),     // open_fee
        pos!(0.96),     // close_fee
    );

    let greeks = strategy.greeks().unwrap();
    let epsilon = dec!(0.001);

    assert_decimal_eq!(greeks.delta, dec!(-0.1148), epsilon);
    assert_decimal_eq!(greeks.gamma, dec!(0.0165), epsilon);
    assert_decimal_eq!(greeks.theta, dec!(-1425.3530), epsilon);
    assert_decimal_eq!(greeks.vega, dec!(3256.2375), epsilon);
    assert_decimal_eq!(greeks.rho, dec!(55.8247), epsilon);
    assert_decimal_eq!(greeks.rho_d, dec!(-63.3206), epsilon);

    assert_relative_eq!(
        strategy.calculate_net_delta().net_delta,
        -0.1148,
        epsilon = 0.001
    );
    assert_relative_eq!(
        strategy.calculate_net_delta().individual_deltas[0],
        0.2492,
        epsilon = 0.001
    );
    assert_relative_eq!(
        strategy.calculate_net_delta().individual_deltas[1],
        -0.1611,
        epsilon = 0.001
    );
    assert!(!strategy.is_delta_neutral());
    assert_eq!(strategy.suggest_delta_adjustments().len(), 2);

    assert_eq!(
        strategy.suggest_delta_adjustments()[0],
        BuyOptions {
            quantity: Positive::new_decimal(Decimal::from_str("0.921345173469528").unwrap())
                .unwrap(),
            strike: pos!(2800.0),
            option_type: OptionStyle::Call
        }
    );

    Ok(())
}
