/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 21/8/24
******************************************************************************/
use crate::model::option::Options;
use crate::model::types::{ExpirationDate, OptionStyle, OptionType, PositiveF64, Side};
use crate::pos;
use chrono::{NaiveDateTime, TimeZone, Utc};

#[allow(dead_code)]
pub(crate) fn create_sample_option(
    option_style: OptionStyle,
    side: Side,
    underlying_price: f64,
    quantity: PositiveF64,
    strike_price: f64,
    volatility: f64,
) -> Options {
    Options::new(
        OptionType::European,
        side,
        "AAPL".to_string(),
        strike_price,
        ExpirationDate::Days(30.0),
        volatility,
        quantity,
        underlying_price,
        0.05,
        option_style,
        0.01,
        None,
    )
}

#[allow(dead_code)]
pub(crate) fn create_sample_option_with_date(
    option_style: OptionStyle,
    side: Side,
    underlying_price: f64,
    quantity: PositiveF64,
    strike_price: f64,
    volatility: f64,
    naive_date: NaiveDateTime,
) -> Options {
    Options::new(
        OptionType::European,
        side,
        "AAPL".to_string(),
        strike_price,
        ExpirationDate::DateTime(Utc.from_utc_datetime(&naive_date)),
        volatility,
        quantity,
        underlying_price,
        0.05,
        option_style,
        0.01,
        None,
    )
}

#[allow(dead_code)]
pub(crate) fn create_sample_option_simplest(option_style: OptionStyle, side: Side) -> Options {
    Options::new(
        OptionType::European,
        side,
        "AAPL".to_string(),
        100.0,
        ExpirationDate::Days(30.0),
        0.2,
        pos!(1.0),
        100.0,
        0.05,
        option_style,
        0.01,
        None,
    )
}
