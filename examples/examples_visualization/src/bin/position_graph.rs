/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/8/24
******************************************************************************/
use chrono::Utc;
use optionstratlib::Positive;
use optionstratlib::model::position::Position;
use optionstratlib::model::types::{OptionStyle, OptionType, Side};
use optionstratlib::pos;
use optionstratlib::visualization::utils::{Graph, GraphBackend};
use optionstratlib::{ExpirationDate, Options};
use rust_decimal_macros::dec;
use std::error::Error;

fn create_sample_option() -> Options {
    Options::new(
        OptionType::European,
        Side::Long,
        "AAPL".to_string(),
        pos!(100.0),
        ExpirationDate::Days(pos!(30.0)),
        pos!(0.2),
        pos!(1.0),
        pos!(105.0),
        dec!(0.05),
        OptionStyle::Call,
        Positive::ZERO,
        None,
    )
}
fn main() -> Result<(), Box<dyn Error>> {
    let position = Position::new(
        create_sample_option(),
        pos!(5.71),
        Utc::now(),
        Positive::ONE,
        Positive::ONE,
    );

    position.graph(
        GraphBackend::Bitmap {
            file_path: "Draws/Position/pnl_at_expiration_chart.png",
            size: (1400, 933),
        },
        20,
    )?;

    Ok(())
}
