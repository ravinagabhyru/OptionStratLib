use chrono::Utc;
use optionstratlib::greeks::Greeks;
use optionstratlib::model::position::Position;
use optionstratlib::model::types::{OptionStyle, OptionType, Side};
use optionstratlib::strategies::custom::CustomStrategy;
use optionstratlib::utils::setup_logger;
use optionstratlib::{ExpirationDate, Options, Positive};
use optionstratlib::{assert_decimal_eq, pos};
use rust_decimal_macros::dec;
use std::error::Error;

#[test]
#[ignore]
fn test_custom_strategy_integration() -> Result<(), Box<dyn Error>> {
    setup_logger();

    // Define common parameters
    let underlying_price = pos!(2340.0);
    let underlying_symbol = "GAS".to_string();
    let expiration = ExpirationDate::Days(pos!(6.0));
    let implied_volatility = pos!(0.73);
    let risk_free_rate = dec!(0.05);
    let dividend_yield = Positive::ZERO;

    // Create positions
    let positions = vec![
        Position::new(
            Options::new(
                OptionType::European,
                Side::Short,
                underlying_symbol.clone(),
                pos!(2100.0),
                expiration,
                implied_volatility,
                pos!(2.0),
                underlying_price,
                risk_free_rate,
                OptionStyle::Call,
                dividend_yield,
                None,
            ),
            pos!(192.0),
            Utc::now(),
            pos!(7.51),
            pos!(7.51),
        ),
        Position::new(
            Options::new(
                OptionType::European,
                Side::Short,
                underlying_symbol.clone(),
                pos!(2250.0),
                expiration,
                implied_volatility,
                pos!(2.0),
                underlying_price,
                risk_free_rate,
                OptionStyle::Call,
                dividend_yield,
                None,
            ),
            pos!(88.0),
            Utc::now(),
            pos!(6.68),
            pos!(6.68),
        ),
        Position::new(
            Options::new(
                OptionType::European,
                Side::Short,
                underlying_symbol.clone(),
                pos!(2500.0),
                expiration,
                implied_volatility,
                pos!(1.0),
                underlying_price,
                risk_free_rate,
                OptionStyle::Put,
                dividend_yield,
                None,
            ),
            pos!(55.0),
            Utc::now(),
            pos!(6.68),
            pos!(6.68),
        ),
        Position::new(
            Options::new(
                OptionType::European,
                Side::Short,
                underlying_symbol.clone(),
                pos!(2150.0),
                expiration,
                implied_volatility,
                pos!(2.5),
                underlying_price,
                risk_free_rate,
                OptionStyle::Put,
                dividend_yield,
                None,
            ),
            pos!(21.0),
            Utc::now(),
            pos!(4.91),
            pos!(4.91),
        ),
    ];

    let strategy = CustomStrategy::new(
        "Custom Strategy".to_string(),
        underlying_symbol,
        "Example of a custom strategy".to_string(),
        underlying_price,
        positions,
        pos!(0.01),
        5,
        pos!(0.1),
    );

    let greeks = strategy.greeks().unwrap();
    let epsilon = dec!(0.001);

    assert_decimal_eq!(greeks.delta, dec!(-1.9757), epsilon);
    assert_decimal_eq!(greeks.gamma, dec!(0.0093), epsilon);
    assert_decimal_eq!(greeks.theta, dec!(-13818.8979), epsilon);
    assert_decimal_eq!(greeks.vega, dec!(1642.8158), epsilon);
    assert_decimal_eq!(greeks.rho, dec!(59.0889), epsilon);
    assert_decimal_eq!(greeks.rho_d, dec!(-75.9988), epsilon);

    Ok(())
}
