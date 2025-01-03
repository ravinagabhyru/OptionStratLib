use approx::assert_relative_eq;
use optionstratlib::greeks::equations::Greeks;
use optionstratlib::model::types::{ExpirationDate, OptionStyle};
use optionstratlib::strategies::bull_call_spread::BullCallSpread;
use optionstratlib::strategies::delta_neutral::DeltaAdjustment::SellOptions;
use optionstratlib::strategies::delta_neutral::DeltaNeutrality;
use optionstratlib::utils::setup_logger;
use optionstratlib::{assert_decimal_eq, pos, Positive};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::error::Error;
use std::str::FromStr;

#[test]
fn test_bull_call_spread_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    // Define inputs for the BullCallSpread strategy
    let underlying_price = pos!(5781.88);

    let strategy = BullCallSpread::new(
        "SP500".to_string(),
        underlying_price, // underlying_price
        pos!(5750.0),     // long_strike_itm
        pos!(5820.0),     // short_strike
        ExpirationDate::Days(pos!(2.0)),
        pos!(0.18),     // implied_volatility
        dec!(0.05),     // risk_free_rate
        Positive::ZERO, // dividend_yield
        pos!(2.0),      // long quantity
        85.04,          // premium_long
        29.85,          // premium_short
        0.78,           // open_fee_long
        0.78,           // open_fee_long
        0.73,           // close_fee_long
        0.73,           // close_fee_short
    );
    let greeks = strategy.greeks();
    let epsilon = dec!(0.001);

    assert_decimal_eq!(greeks.delta, dec!(0.7004), epsilon);
    assert_decimal_eq!(greeks.gamma, dec!(0.0186), epsilon);
    assert_decimal_eq!(greeks.theta, dec!(-10685.1215), epsilon);
    assert_decimal_eq!(greeks.vega, dec!(848.6626), epsilon);
    assert_decimal_eq!(greeks.rho, dec!(62.0955), epsilon);
    assert_decimal_eq!(greeks.rho_d, dec!(-62.8208), epsilon);

    assert_relative_eq!(
        strategy.calculate_net_delta().net_delta,
        0.7004,
        epsilon = 0.001
    );
    assert_relative_eq!(
        strategy.calculate_net_delta().individual_deltas[0],
        1.3416,
        epsilon = 0.001
    );
    assert_relative_eq!(
        strategy.calculate_net_delta().individual_deltas[1],
        -0.6412,
        epsilon = 0.001
    );
    assert!(!strategy.is_delta_neutral());
    assert_eq!(strategy.suggest_delta_adjustments().len(), 1);

    assert_eq!(
        strategy.suggest_delta_adjustments()[0],
        SellOptions {
            quantity: Positive::new_decimal(Decimal::from_str("2.184538786861798").unwrap())
                .unwrap(),
            strike: pos!(5820.0),
            option_type: OptionStyle::Call
        }
    );

    Ok(())
}
