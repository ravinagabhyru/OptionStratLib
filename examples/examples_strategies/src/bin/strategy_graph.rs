/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/8/24
******************************************************************************/
use optionstratlib::model::types::PositiveF64;
use optionstratlib::model::types::{ExpirationDate, PZERO};
use optionstratlib::pos;
use optionstratlib::strategies::base::Strategies;
use optionstratlib::strategies::bull_call_spread::BullCallSpread;
use optionstratlib::utils::logger::setup_logger;
use optionstratlib::visualization::utils::Graph;
use std::error::Error;
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger();
    let strategy = BullCallSpread::new(
        "GOLD".to_string(),
        pos!(2505.8),
        pos!(2460.0),
        pos!(2515.0),
        ExpirationDate::Days(30.0),
        0.2,
        0.05,
        0.0,
        pos!(1.0),
        27.26,
        5.33,
        0.58,
        0.58,
        0.55,
        0.54,
    );
    let price_range: Vec<PositiveF64> = (2400..2600)
        .map(|x| PositiveF64::new(x as f64).unwrap())
        .collect();
    info!("Title: {}", strategy.title());
    info!("Break Even {:?}", strategy.break_even());
    info!("Net Premium Received: {}", strategy.net_premium_received());
    info!("Max Profit: {}", strategy.max_profit().unwrap_or(PZERO));
    info!("Max Loss: {}", strategy.max_loss().unwrap_or(PZERO));
    info!("Total Cost: {}", strategy.total_cost());

    // Generate the intrinsic value graph
    strategy.graph(
        &price_range,
        "Draws/Strategy/bull_call_spread_value_chart.png",
        20,
        (1400, 933),
    )?;
    Ok(())
}
