/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/8/24
******************************************************************************/
use chrono::Utc;
use optionstratlib::model::option::Options;
use optionstratlib::model::position::Position;
use optionstratlib::model::types::{ExpirationDate, OptionStyle, OptionType, Side};
use optionstratlib::visualization::utils::Graph;
use std::error::Error;

fn create_sample_option() -> Options {
    Options::new(
        OptionType::European,
        Side::Long,
        "AAPL".to_string(),
        100.0,
        ExpirationDate::Days(30.0),
        0.2,
        1,
        105.0,
        0.05,
        OptionStyle::Call,
        0.0,
        None,
    )
}
fn main() -> Result<(), Box<dyn Error>> {
    let position = Position::new(create_sample_option(), 5.0, Utc::now(), 1.0, 1.0);

    let price_range: Vec<f64> = (50..150).map(|x| x as f64).collect();

    position.graph(&price_range, "Draws/Position/pnl_at_expiration_chart.png")?;

    Ok(())
}
