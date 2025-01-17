use optionstratlib::curves::construction::CurveConstructionMethod;
use optionstratlib::curves::visualization::Plottable;
use optionstratlib::curves::{Curve, Point2D};
use optionstratlib::greeks::Greeks;
use optionstratlib::utils::setup_logger;
use optionstratlib::{pos, ExpirationDate, OptionStyle, OptionType, Options, Positive, Side};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::error::Error;

fn get_option(strike: &Positive) -> Options {
    Options::new(
        OptionType::European,
        Side::Long,
        "XYZ".parse().unwrap(),
        *strike,
        ExpirationDate::Days(pos!(365.0)),
        pos!(0.10),
        pos!(1.0),
        pos!(50.0),
        Decimal::ZERO,
        OptionStyle::Call,
        Positive::ZERO,
        None,
    )
}
fn main() -> Result<(), Box<dyn Error>> {
    setup_logger();

    let parametric_curve = Curve::construct(CurveConstructionMethod::Parametric {
        f: Box::new(|t| {
            let option = get_option(&Positive::new_decimal(t).unwrap());
            let value = option.theta().unwrap();
            let point = Point2D::new(t, value);
            Ok(point)
        }),
        t_start: dec!(20.0),
        t_end: dec!(80),
        steps: 100,
    })?;

    parametric_curve
        .plot()
        .title("Theta Curve")
        .x_label("Asset value")
        .y_label("theta")
        .line_width(1)
        .save("./Draws/Curves/theta_curve.png")?;

    Ok(())
}
