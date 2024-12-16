/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 25/9/24
******************************************************************************/

use optionstratlib::greeks::equations::Greeks;
use optionstratlib::model::types::PositiveF64;
use optionstratlib::model::types::{ExpirationDate, PZERO};
use optionstratlib::pos;
use optionstratlib::strategies::base::Strategies;
use optionstratlib::strategies::butterfly_spread::LongButterflySpread;
use optionstratlib::utils::logger::setup_logger;
use optionstratlib::visualization::utils::Graph;
use std::error::Error;
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger();

    let underlying_price = pos!(5795.88);

    let strategy = LongButterflySpread::new(
        "SP500".to_string(),
        underlying_price, // underlying_price
        pos!(5710.0),     // long_strike_itm
        pos!(5780.0),     // short_strike
        pos!(5850.0),     // long_strike_otm
        ExpirationDate::Days(2.0),
        0.18,      // implied_volatility
        0.05,      // risk_free_rate
        0.0,       // dividend_yield
        pos!(1.0), // long quantity
        113.30,    // premium_long
        64.20,     // premium_short
        31.65,     // open_fee_long
        0.07,      // open_fee_long
    );

    let price_range = strategy.best_range_to_show(pos!(1.0)).unwrap();

    info!("Title: {}", strategy.title());
    info!("Break Even Points: {:?}", strategy.break_even_points);
    info!(
        "Net Premium Received: ${:.2}",
        strategy.net_premium_received()
    );
    info!("Max Profit: ${:.2}", strategy.max_profit().unwrap_or(PZERO));
    info!("Max Loss: ${:0.2}", strategy.max_loss().unwrap_or(PZERO));
    info!("Total Fees: ${:.2}", strategy.fees());
    info!("Profit Area: {:.2}%", strategy.profit_area());
    info!("Profit Ratio: {:.2}%", strategy.profit_ratio());

    // Generate the profit/loss graph
    strategy.graph(
        &price_range,
        "Draws/Strategy/long_butterfly_spread_profit_loss_chart.png",
        20,
        (1400, 933),
    )?;

    info!("Greeks:  {:#?}", strategy.greeks());

    Ok(())
}
