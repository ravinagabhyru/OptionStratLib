/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 25/9/24
******************************************************************************/
use optionstratlib::model::types::ExpirationDate;
use optionstratlib::strategies::base::Strategies;
use optionstratlib::strategies::ratio_call_spread::RatioCallSpread;
use optionstratlib::utils::logger::setup_logger;
use optionstratlib::visualization::utils::Graph;
use std::error::Error;
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger();

    let underlying_price = 2646.9;

    let strategy = RatioCallSpread::new(
        "GOLD".to_string(),
        underlying_price, // underlying_price
        2600.0,           // long_strike_itm
        2700.0,           // long_strike_otm
        2650.0,           // short_strike
        ExpirationDate::Days(2.0),
        0.15, // implied_volatility
        0.05, // risk_free_rate
        0.0,  // dividend_yield
        1,    // long quantity
        2,    // short_quantity
        93.0, // premium_long
        48.0, // premium_short
        65.9, // open_fee_long
        0.78, // close_fee_long
        0.78, // open_fee_short
        0.73, // close_fee_short
        0.73, // close_fee_short
    );

    let price_range: Vec<f64> = (2450..=2850).map(|x| x as f64).collect();
    // let range = strategy.break_even_points[1] - strategy.break_even_points[0];

    info!("Title: {}", strategy.title());
    info!("Break Even Points: {:?}", strategy.break_even_points);
    info!(
        "Net Premium Received: ${:.2}",
        strategy.net_premium_received()
    );
    info!("Max Profit: ${:.2}", strategy.max_profit());
    info!("Max Loss: ${}", strategy.max_loss());
    info!("Total Fees: ${:.2}", strategy.fees());
    // info!(
    //     "Range of Profit: ${:.2} {:.2}%",
    //     range,
    //     (range / 2.0) / underlying_price * 100.0
    // );
    info!("Profit Area: {:.2}%", strategy.area());

    // Generate the profit/loss graph
    strategy.graph(
        &price_range,
        "Draws/Strategy/ratio_call_spread_profit_loss_chart.png",
        20,
        (1400, 933),
        (10, 30),
        15,
    )?;

    Ok(())
}