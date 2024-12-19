use optionstratlib::chains::chain::OptionChain;
use optionstratlib::constants::ZERO;
use optionstratlib::model::types::PositiveF64;
use optionstratlib::model::types::{ExpirationDate, PZERO};
use optionstratlib::pos;
use optionstratlib::strategies::base::{Optimizable, Strategies};
use optionstratlib::strategies::butterfly_spread::ShortButterflySpread;
use optionstratlib::strategies::utils::FindOptimalSide;
use optionstratlib::utils::logger::setup_logger;
use optionstratlib::visualization::utils::Graph;
use std::error::Error;
use tracing::{debug, info};

fn main() -> Result<(), Box<dyn Error>> {
    setup_logger();

    let option_chain =
        OptionChain::load_from_json("./examples/Chains/SP500-18-oct-2024-5781.88.json")?;
    let underlying_price = option_chain.underlying_price;
    let mut strategy = ShortButterflySpread::new(
        "SP500".to_string(),
        underlying_price, // underlying_price
        PZERO,            // short_strike_itm
        PZERO,            // middle_strike
        PZERO,            // short_strike_otm
        ExpirationDate::Days(5.0),
        ZERO,      // implied_volatility
        ZERO,      // risk_free_rate
        ZERO,      // dividend_yield
        pos!(1.0), // quantity
        ZERO,      // premium_short_call
        ZERO,      // premium_short_put
        ZERO,      // open_fee_short_call
        4.0,       // close_fee_short_call
    );
    strategy.best_area(&option_chain, FindOptimalSide::All);
    debug!("Option Chain: {}", option_chain);
    debug!("Strategy:  {:#?}", strategy);

    let price_range = strategy.best_range_to_show(pos!(1.0)).unwrap();
    let range = strategy.range_of_profit().unwrap_or(PZERO);
    info!("Title: {}", strategy.title());
    info!("Break Even Points: {:?}", strategy.break_even_points);
    info!(
        "Net Premium Received: ${:.2}",
        strategy.net_premium_received()
    );
    info!("Max Profit: ${:.2}", strategy.max_profit().unwrap_or(PZERO));
    info!("Max Loss: ${:0.2}", strategy.max_loss().unwrap_or(PZERO));
    info!("Total Fees: ${:.2}", strategy.fees());
    info!(
        "Range of Profit: ${:.2} {:.2}%",
        range,
        (range / 2.0) / underlying_price * 100.0
    );
    info!("Profit Area: {:.2}%", strategy.profit_area());

    if strategy.profit_area() > ZERO {
        debug!("Strategy:  {:#?}", strategy);
        strategy.graph(
            &price_range,
            "Draws/Strategy/short_butterfly_spread_profit_loss_chart_best_area.png",
            20,
            (1400, 933),
        )?;
    }

    Ok(())
}