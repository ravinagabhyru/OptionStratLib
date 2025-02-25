/*
Long Butterfly Spread Strategy

A long butterfly spread involves:
1. Buy one call at a lower strike price (ITM)
2. Sell two calls at a middle strike price (ATM)
3. Buy one call at a higher strike price (OTM)

All options have the same expiration date.

Key characteristics:
- Limited profit potential
- Limited risk
- Profit is highest when price is exactly at middle strike at expiration
- Maximum loss is limited to the net premium paid
- All options must have same expiration date
*/
use super::base::{Optimizable, Positionable, Strategies, StrategyType, Validable};
use crate::chains::chain::OptionChain;
use crate::chains::utils::OptionDataGroup;
use crate::chains::StrategyLegs;
use crate::constants::{DARK_BLUE, DARK_GREEN, ZERO};
use crate::error::position::PositionError;
use crate::error::probability::ProbabilityError;
use crate::error::strategies::{ProfitLossErrorKind, StrategyError};
use crate::error::GreeksError;
use crate::greeks::Greeks;
use crate::model::position::Position;
use crate::model::types::{ExpirationDate, OptionStyle, OptionType, Side};
use crate::model::utils::mean_and_std;
use crate::model::ProfitLossRange;
use crate::pricing::payoff::Profit;
use crate::strategies::delta_neutral::{
    DeltaAdjustment, DeltaInfo, DeltaNeutrality, DELTA_THRESHOLD,
};
use crate::strategies::probabilities::{ProbabilityAnalysis, VolatilityAdjustment};
use crate::strategies::utils::{FindOptimalSide, OptimizationCriteria};
use crate::visualization::model::{ChartPoint, ChartVerticalLine, LabelOffsetType};
use crate::visualization::utils::Graph;
use crate::Options;
use crate::{pos, Positive};
use chrono::Utc;
use num_traits::ToPrimitive;
use plotters::prelude::full_palette::ORANGE;
use plotters::prelude::{ShapeStyle, RED};
use rust_decimal::Decimal;
use std::error::Error;
use tracing::{debug, info};

const LONG_BUTTERFLY_DESCRIPTION: &str =
    "A long butterfly spread is created by buying one call at a lower strike price, \
    selling two calls at a middle strike price, and buying one call at a higher strike price, \
    all with the same expiration date. This strategy profits when the underlying price stays \
    near the middle strike price at expiration.";

#[derive(Clone, Debug)]
pub struct LongButterflySpread {
    pub name: String,
    pub kind: StrategyType,
    pub description: String,
    pub break_even_points: Vec<Positive>,
    long_call_low: Position,
    short_calls: Position,
    long_call_high: Position,
}

impl LongButterflySpread {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        underlying_symbol: String,
        underlying_price: Positive,
        low_strike: Positive,
        middle_strike: Positive,
        high_strike: Positive,
        expiration: ExpirationDate,
        implied_volatility: Positive,
        risk_free_rate: Decimal,
        dividend_yield: Positive,
        quantity: Positive,
        premium_low: Positive,
        premium_middle: Positive,
        premium_high: Positive,
        open_fee_short_call: Positive,
        close_fee_short_call: Positive,
        open_fee_long_call_low: Positive,
        close_fee_long_call_low: Positive,
        open_fee_long_call_high: Positive,
        close_fee_long_call_high: Positive,
    ) -> Self {
        let mut strategy = LongButterflySpread {
            name: "Long Butterfly".to_string(),
            kind: StrategyType::LongButterflySpread,
            description: LONG_BUTTERFLY_DESCRIPTION.to_string(),
            break_even_points: Vec::new(),
            long_call_low: Position::default(),
            short_calls: Position::default(),
            long_call_high: Position::default(),
        };

        // Create two short calls at middle strike
        let short_calls = Options::new(
            OptionType::European,
            Side::Short,
            underlying_symbol.clone(),
            middle_strike,
            expiration.clone(),
            implied_volatility,
            quantity * 2.0, // Double quantity for middle strike
            underlying_price,
            risk_free_rate,
            OptionStyle::Call,
            dividend_yield,
            None,
        );
        strategy.short_calls = Position::new(
            short_calls,
            premium_middle,
            Utc::now(),
            open_fee_short_call,
            close_fee_short_call,
        );

        // Create long call at lower strike
        let long_call_low = Options::new(
            OptionType::European,
            Side::Long,
            underlying_symbol.clone(),
            low_strike,
            expiration.clone(),
            implied_volatility,
            quantity,
            underlying_price,
            risk_free_rate,
            OptionStyle::Call,
            dividend_yield,
            None,
        );
        strategy.long_call_low = Position::new(
            long_call_low,
            premium_low,
            Utc::now(),
            open_fee_long_call_low,
            close_fee_long_call_low,
        );

        // Create long call at higher strike
        let long_call_high = Options::new(
            OptionType::European,
            Side::Long,
            underlying_symbol,
            high_strike,
            expiration,
            implied_volatility,
            quantity,
            underlying_price,
            risk_free_rate,
            OptionStyle::Call,
            dividend_yield,
            None,
        );
        strategy.long_call_high = Position::new(
            long_call_high,
            premium_high,
            Utc::now(),
            open_fee_long_call_high,
            close_fee_long_call_high,
        );

        strategy.validate();

        let left_net_value = strategy
            .calculate_profit_at(strategy.long_call_low.option.strike_price)
            .unwrap()
            / quantity;
        let right_net_value = strategy
            .calculate_profit_at(strategy.long_call_high.option.strike_price)
            .unwrap()
            / quantity;

        if left_net_value <= Decimal::ZERO {
            strategy
                .break_even_points
                .push((strategy.long_call_low.option.strike_price - left_net_value).round_to(2));
        }

        if right_net_value <= Decimal::ZERO {
            strategy
                .break_even_points
                .push((strategy.long_call_high.option.strike_price + right_net_value).round_to(2));
        }

        strategy
    }
}

impl Validable for LongButterflySpread {
    fn validate(&self) -> bool {
        if !self.long_call_low.validate() {
            debug!("Long call (low strike) is invalid");
            return false;
        }
        if !self.short_calls.validate() {
            debug!("Short calls (middle strike) are invalid");
            return false;
        }
        if !self.long_call_high.validate() {
            debug!("Long call (high strike) is invalid");
            return false;
        }

        if self.long_call_low.option.strike_price >= self.short_calls.option.strike_price {
            debug!("Low strike must be lower than middle strike");
            return false;
        }
        if self.short_calls.option.strike_price >= self.long_call_high.option.strike_price {
            debug!("Middle strike must be lower than high strike");
            return false;
        }

        if self.short_calls.option.quantity != self.long_call_low.option.quantity * 2.0 {
            debug!("Middle strike quantity must be double the wing quantities");
            return false;
        }
        if self.long_call_low.option.quantity != self.long_call_high.option.quantity {
            debug!("Wing quantities must be equal");
            return false;
        }

        true
    }
}

impl Positionable for LongButterflySpread {
    fn add_position(&mut self, position: &Position) -> Result<(), PositionError> {
        match &position.option.side {
            Side::Long => {
                // short_calls should be inserted first
                if position.option.strike_price < self.short_calls.option.strike_price {
                    self.long_call_low = position.clone();
                    Ok(())
                } else {
                    self.long_call_high = position.clone();
                    Ok(())
                }
            }
            Side::Short => {
                self.short_calls = position.clone();
                Ok(())
            }
        }
    }

    fn get_positions(&self) -> Result<Vec<&Position>, PositionError> {
        Ok(vec![
            &self.long_call_low,
            &self.short_calls,
            &self.long_call_high,
        ])
    }
}

impl Strategies for LongButterflySpread {
    fn get_underlying_price(&self) -> Positive {
        self.long_call_low.option.underlying_price
    }

    fn max_profit(&self) -> Result<Positive, StrategyError> {
        let profit = self.calculate_profit_at(self.short_calls.option.strike_price)?;
        if profit > Decimal::ZERO {
            Ok(profit.into())
        } else {
            Err(StrategyError::ProfitLossError(
                ProfitLossErrorKind::MaxProfitError {
                    reason: "max_profit is negative".to_string(),
                },
            ))
        }
    }

    fn max_loss(&self) -> Result<Positive, StrategyError> {
        let left_loss = self.calculate_profit_at(self.long_call_low.option.strike_price)?;
        let right_loss = self.calculate_profit_at(self.long_call_high.option.strike_price)?;
        let max_loss = left_loss.min(right_loss);
        if max_loss > Decimal::ZERO {
            Err(StrategyError::ProfitLossError(
                ProfitLossErrorKind::MaxLossError {
                    reason: "Max loss is negative".to_string(),
                },
            ))
        } else {
            Ok(max_loss.abs().into())
        }
    }

    fn profit_area(&self) -> Result<Decimal, StrategyError> {
        let high = self.max_profit().unwrap_or(Positive::ZERO);
        let break_even_points = self.get_break_even_points()?;

        let base = if break_even_points.len() == 2 {
            break_even_points[1] - break_even_points[0]
        } else {
            let break_even_point = break_even_points[0];

            if break_even_point < self.short_calls.option.strike_price {
                self.calculate_profit_at(self.long_call_high.option.strike_price)?
                    .into()
            } else {
                self.calculate_profit_at(self.long_call_low.option.strike_price)?
                    .into()
            }
        };
        Ok((high * base / 200.0).into())
    }

    fn profit_ratio(&self) -> Result<Decimal, StrategyError> {
        let max_profit = self.max_profit().unwrap_or(Positive::ZERO);
        let max_loss = self.max_loss().unwrap_or(Positive::ZERO);
        match (max_profit, max_loss) {
            (value, _) if value == Positive::ZERO => Ok(Decimal::ZERO),
            (_, value) if value == Positive::ZERO => Ok(Decimal::MAX),
            _ => Ok((max_profit / max_loss * 100.0).into()),
        }
    }

    fn get_break_even_points(&self) -> Result<&Vec<Positive>, StrategyError> {
        Ok(&self.break_even_points)
    }
}

impl Optimizable for LongButterflySpread {
    type Strategy = LongButterflySpread;

    fn filter_combinations<'a>(
        &'a self,
        option_chain: &'a OptionChain,
        side: FindOptimalSide,
    ) -> impl Iterator<Item = OptionDataGroup<'a>> {
        let underlying_price = self.get_underlying_price();
        let strategy = self.clone();
        option_chain
            .get_triple_iter()
            // Filter out invalid combinations based on FindOptimalSide
            .filter(move |(long_low, short, long_high)| {
                long_low.is_valid_optimal_side(underlying_price, &side)
                    && short.is_valid_optimal_side(underlying_price, &side)
                    && long_high.is_valid_optimal_side(underlying_price, &side)
            })
            .filter(move |(long_low, short, long_high)| {
                long_low.strike_price < short.strike_price
                    && short.strike_price < long_high.strike_price
            })
            // Filter out options with invalid bid/ask prices
            .filter(|(long_low, short, long_high)| {
                long_low.call_ask.unwrap_or(Positive::ZERO) > Positive::ZERO
                    && short.call_bid.unwrap_or(Positive::ZERO) > Positive::ZERO
                    && long_high.call_ask.unwrap_or(Positive::ZERO) > Positive::ZERO
            })
            // Filter out options that don't meet strategy constraints
            .filter(move |(long_low, short, long_high)| {
                let legs = StrategyLegs::ThreeLegs {
                    first: long_low,
                    second: short,
                    third: long_high,
                };
                let strategy = strategy.create_strategy(option_chain, &legs);
                strategy.validate() && strategy.max_profit().is_ok() && strategy.max_loss().is_ok()
            })
            // Map to OptionDataGroup
            .map(move |(long_low, short, long_high)| {
                OptionDataGroup::Three(long_low, short, long_high)
            })
    }

    fn find_optimal(
        &mut self,
        option_chain: &OptionChain,
        side: FindOptimalSide,
        criteria: OptimizationCriteria,
    ) {
        let mut best_value = Decimal::MIN;
        let strategy_clone = self.clone();
        let options_iter = strategy_clone.filter_combinations(option_chain, side);

        for option_data_group in options_iter {
            // Unpack the OptionDataGroup into individual options
            let (long_low, short, long_high) = match option_data_group {
                OptionDataGroup::Three(first, second, third) => (first, second, third),
                _ => panic!("Invalid OptionDataGroup"),
            };

            let legs = StrategyLegs::ThreeLegs {
                first: long_low,
                second: short,
                third: long_high,
            };
            let strategy = self.create_strategy(option_chain, &legs);
            // Calculate the current value based on the optimization criteria
            let current_value = match criteria {
                OptimizationCriteria::Ratio => strategy.profit_ratio().unwrap(),
                OptimizationCriteria::Area => strategy.profit_area().unwrap(),
            };

            if current_value > best_value {
                // Update the best value and replace the current strategy
                info!("Found better value: {}", current_value);
                best_value = current_value;
                *self = strategy.clone();
            }
        }
    }

    fn create_strategy(&self, chain: &OptionChain, legs: &StrategyLegs) -> Self::Strategy {
        match legs {
            StrategyLegs::ThreeLegs {
                first: low_strike,
                second: middle_strike,
                third: high_strike,
            } => LongButterflySpread::new(
                chain.symbol.clone(),
                chain.underlying_price,
                low_strike.strike_price,
                middle_strike.strike_price,
                high_strike.strike_price,
                self.long_call_low.option.expiration_date.clone(),
                middle_strike.implied_volatility.unwrap() / 100.0,
                self.long_call_low.option.risk_free_rate,
                self.long_call_low.option.dividend_yield,
                self.long_call_low.option.quantity,
                low_strike.call_ask.unwrap(),
                middle_strike.call_bid.unwrap(),
                high_strike.call_ask.unwrap(),
                self.short_calls.open_fee,
                self.short_calls.close_fee,
                self.long_call_low.open_fee,
                self.long_call_low.close_fee,
                self.long_call_high.open_fee,
                self.long_call_high.close_fee,
            ),
            _ => panic!("Invalid number of legs for Long Butterfly strategy"),
        }
    }
}

impl Profit for LongButterflySpread {
    fn calculate_profit_at(&self, price: Positive) -> Result<Decimal, Box<dyn Error>> {
        let price = Some(&price);
        Ok(self.long_call_low.pnl_at_expiration(&price)?
            + self.short_calls.pnl_at_expiration(&price)?
            + self.long_call_high.pnl_at_expiration(&price)?)
    }
}

impl Graph for LongButterflySpread {
    fn title(&self) -> String {
        let strategy_title = format!(
            "{:?} Strategy on {} Size {}:",
            self.kind,
            self.long_call_low.option.underlying_symbol,
            self.long_call_low.option.quantity
        );

        let leg_titles = [
            format!(
                "Long Call Low Strike: ${}",
                self.long_call_low.option.strike_price
            ),
            format!(
                "Short Calls Middle Strike: ${}",
                self.short_calls.option.strike_price
            ),
            format!(
                "Long Call High Strike: ${}",
                self.long_call_high.option.strike_price
            ),
            format!(
                "Expire: {}",
                self.long_call_low
                    .option
                    .expiration_date
                    .get_date_string()
                    .unwrap()
            ),
        ];

        format!("{}\n\t{}", strategy_title, leg_titles.join("\n\t"))
    }

    fn get_vertical_lines(&self) -> Vec<ChartVerticalLine<f64, f64>> {
        vec![ChartVerticalLine {
            x_coordinate: self.long_call_low.option.underlying_price.to_f64(),
            y_range: (-50000.0, 50000.0),
            label: format!(
                "Current Price: {}",
                self.long_call_low.option.underlying_price
            ),
            label_offset: (5.0, 5.0),
            line_color: ORANGE,
            label_color: ORANGE,
            line_style: ShapeStyle::from(&ORANGE).stroke_width(2),
            font_size: 18,
        }]
    }

    fn get_points(&self) -> Vec<ChartPoint<(f64, f64)>> {
        let mut points = Vec::new();
        let max_profit = self.max_profit().unwrap_or(Positive::ZERO);

        let left_loss = self
            .calculate_profit_at(self.long_call_low.option.strike_price)
            .unwrap()
            .to_f64()
            .unwrap();
        let right_loss = self
            .calculate_profit_at(self.long_call_high.option.strike_price)
            .unwrap()
            .to_f64()
            .unwrap();

        let font_size = 24;
        // Break-even points
        points.extend(
            self.break_even_points
                .iter()
                .enumerate()
                .map(|(i, &price)| ChartPoint {
                    coordinates: (price.to_f64(), 0.0),
                    label: format!(
                        "Break Even {}: {:.2}",
                        if i == 0 { "Lower" } else { "Upper" },
                        price
                    ),
                    label_offset: LabelOffsetType::Relative(3.0, 3.0),
                    point_color: DARK_BLUE,
                    label_color: DARK_BLUE,
                    point_size: 5,
                    font_size,
                }),
        );

        // Maximum profit point (at middle strike)
        points.push(ChartPoint {
            coordinates: (
                self.short_calls.option.strike_price.to_f64(),
                max_profit.to_f64(),
            ),
            label: format!("Max Profit {:.2}", max_profit),
            label_offset: LabelOffsetType::Relative(3.0, 3.0),
            point_color: DARK_GREEN,
            label_color: DARK_GREEN,
            point_size: 5,
            font_size,
        });

        let left_color = if left_loss > ZERO { DARK_GREEN } else { RED };

        // Maximum loss points (at wing strikes)
        points.push(ChartPoint {
            coordinates: (self.long_call_low.option.strike_price.to_f64(), left_loss),
            label: format!("Left Loss {:.2}", left_loss),
            label_offset: LabelOffsetType::Relative(-30.0, -3.0),
            point_color: left_color,
            label_color: left_color,
            point_size: 5,
            font_size,
        });

        let right_color = if right_loss > ZERO { DARK_GREEN } else { RED };

        points.push(ChartPoint {
            coordinates: (self.long_call_high.option.strike_price.to_f64(), right_loss),
            label: format!("Right Loss {:.2}", right_loss),
            label_offset: LabelOffsetType::Relative(3.0, -3.0),
            point_color: right_color,
            label_color: right_color,
            point_size: 5,
            font_size,
        });

        // Current price point
        points.push(self.get_point_at_price(self.long_call_low.option.underlying_price));

        points
    }
}

impl ProbabilityAnalysis for LongButterflySpread {
    fn get_expiration(&self) -> Result<ExpirationDate, ProbabilityError> {
        Ok(self.long_call_low.option.expiration_date.clone())
    }

    fn get_risk_free_rate(&self) -> Option<Decimal> {
        Some(self.long_call_low.option.risk_free_rate)
    }

    fn get_profit_ranges(&self) -> Result<Vec<ProfitLossRange>, ProbabilityError> {
        let break_even_points = self.get_break_even_points()?;

        let (mean_volatility, std_dev) = mean_and_std(vec![
            self.long_call_low.option.implied_volatility,
            self.short_calls.option.implied_volatility,
            self.long_call_high.option.implied_volatility,
        ]);

        let mut profit_range = ProfitLossRange::new(
            Some(break_even_points[0]),
            Some(break_even_points[1]),
            pos!(self.max_profit()?.to_f64()),
        )?;

        profit_range.calculate_probability(
            self.get_underlying_price(),
            Some(VolatilityAdjustment {
                base_volatility: mean_volatility,
                std_dev_adjustment: std_dev,
            }),
            None,
            self.get_expiration()?,
            self.get_risk_free_rate(),
        )?;

        Ok(vec![profit_range])
    }

    fn get_loss_ranges(&self) -> Result<Vec<ProfitLossRange>, ProbabilityError> {
        let mut ranges = Vec::new();
        let break_even_points = self.get_break_even_points()?;

        let (mean_volatility, std_dev) = mean_and_std(vec![
            self.long_call_low.option.implied_volatility,
            self.short_calls.option.implied_volatility,
            self.long_call_high.option.implied_volatility,
        ]);

        let volatility_adjustment = Some(VolatilityAdjustment {
            base_volatility: mean_volatility,
            std_dev_adjustment: std_dev,
        });

        let mut lower_loss_range = ProfitLossRange::new(
            Some(self.long_call_low.option.strike_price),
            Some(break_even_points[0]),
            pos!(self.max_loss()?.to_f64()),
        )?;

        lower_loss_range.calculate_probability(
            self.get_underlying_price(),
            volatility_adjustment.clone(),
            None,
            self.get_expiration()?,
            self.get_risk_free_rate(),
        )?;

        ranges.push(lower_loss_range);

        let mut upper_loss_range = ProfitLossRange::new(
            Some(break_even_points[1]),
            Some(self.long_call_high.option.strike_price),
            pos!(self.max_loss()?.to_f64()),
        )?;

        upper_loss_range.calculate_probability(
            self.get_underlying_price(),
            volatility_adjustment,
            None,
            self.get_expiration()?,
            self.get_risk_free_rate(),
        )?;

        ranges.push(upper_loss_range);

        Ok(ranges)
    }
}

impl Greeks for LongButterflySpread {
    fn get_options(&self) -> Result<Vec<&Options>, GreeksError> {
        Ok(vec![
            &self.long_call_low.option,
            &self.short_calls.option,
            &self.long_call_high.option,
        ])
    }
}

impl DeltaNeutrality for LongButterflySpread {
    fn calculate_net_delta(&self) -> DeltaInfo {
        let long_call_low_delta = self.long_call_low.option.delta();
        let long_call_high_delta = self.long_call_high.option.delta();
        let short_calls_delta = self.short_calls.option.delta();
        let threshold = DELTA_THRESHOLD;
        let l_cl_delta = long_call_low_delta.unwrap();
        let l_ch_delta = long_call_high_delta.unwrap();
        let s_c_delta = short_calls_delta.unwrap();

        let delta = l_cl_delta + l_ch_delta + s_c_delta;
        DeltaInfo {
            net_delta: delta,
            individual_deltas: vec![l_cl_delta, l_ch_delta, s_c_delta],
            is_neutral: (delta).abs() < threshold,
            underlying_price: self.long_call_low.option.underlying_price,
            neutrality_threshold: threshold,
        }
    }

    fn get_atm_strike(&self) -> Positive {
        self.long_call_low.option.underlying_price
    }

    fn generate_delta_reducing_adjustments(&self) -> Vec<DeltaAdjustment> {
        let net_delta = self.calculate_net_delta().net_delta;
        let delta = self.short_calls.option.delta().unwrap();
        let qty = Positive((net_delta.abs() / delta).abs());

        vec![DeltaAdjustment::SellOptions {
            quantity: qty * self.short_calls.option.quantity,
            strike: self.short_calls.option.strike_price,
            option_type: OptionStyle::Call,
        }]
    }

    fn generate_delta_increasing_adjustments(&self) -> Vec<DeltaAdjustment> {
        let net_delta = self.calculate_net_delta().net_delta;
        let delta_low = self.long_call_low.option.delta().unwrap();
        let delta_high = self.long_call_high.option.delta().unwrap();
        let qty_low = Positive((net_delta.abs() / delta_low).abs());
        let qty_high = Positive((net_delta.abs() / delta_high).abs());

        vec![
            DeltaAdjustment::BuyOptions {
                quantity: qty_low * self.long_call_low.option.quantity,
                strike: self.long_call_low.option.strike_price,
                option_type: OptionStyle::Call,
            },
            DeltaAdjustment::BuyOptions {
                quantity: qty_high * self.long_call_high.option.quantity,
                strike: self.long_call_high.option.strike_price,
                option_type: OptionStyle::Call,
            },
        ]
    }
}

/*
Short Butterfly Spread Strategy

A short butterfly spread involves:
1. Sell one call at a lower strike price (ITM)
2. Buy two calls at a middle strike price (ATM)
3. Sell one call at a higher strike price (OTM)

All options have the same expiration date.

Key characteristics:
- Limited profit potential
- Limited risk
- Loss is highest when price is exactly at middle strike at expiration
- Maximum profit is limited to the net premium received
- All options must have same expiration date
*/

const SHORT_BUTTERFLY_DESCRIPTION: &str =
    "A short butterfly spread is created by selling one call at a lower strike price, \
    buying two calls at a middle strike price, and selling one call at a higher strike price, \
    all with the same expiration date. This strategy profits when the underlying price moves \
    significantly away from the middle strike price in either direction.";

#[derive(Clone, Debug)]
pub struct ShortButterflySpread {
    pub name: String,
    pub kind: StrategyType,
    pub description: String,
    pub break_even_points: Vec<Positive>,
    short_call_low: Position,
    long_calls: Position,
    short_call_high: Position,
}

impl ShortButterflySpread {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        underlying_symbol: String,
        underlying_price: Positive,
        low_strike: Positive,
        middle_strike: Positive,
        high_strike: Positive,
        expiration: ExpirationDate,
        implied_volatility: Positive,
        risk_free_rate: Decimal,
        dividend_yield: Positive,
        quantity: Positive,
        premium_low: Positive,
        premium_middle: Positive,
        premium_high: Positive,
        open_fee_long_call: Positive,
        close_fee_long_call: Positive,
        open_fee_short_call_low: Positive,
        close_fee_short_call_low: Positive,
        open_fee_short_call_high: Positive,
        close_fee_short_call_high: Positive,
    ) -> Self {
        let mut strategy = ShortButterflySpread {
            name: "Short Butterfly".to_string(),
            kind: StrategyType::ShortButterflySpread,
            description: SHORT_BUTTERFLY_DESCRIPTION.to_string(),
            break_even_points: Vec::new(),
            short_call_low: Position::default(),
            long_calls: Position::default(),
            short_call_high: Position::default(),
        };

        // Create two long calls at middle strike
        let long_calls = Options::new(
            OptionType::European,
            Side::Long,
            underlying_symbol.clone(),
            middle_strike,
            expiration.clone(),
            implied_volatility,
            quantity * 2.0, // Double quantity for middle strike
            underlying_price,
            risk_free_rate,
            OptionStyle::Call,
            dividend_yield,
            None,
        );
        strategy.long_calls = Position::new(
            long_calls,
            premium_middle,
            Utc::now(),
            open_fee_long_call,
            close_fee_long_call,
        );

        // Create short call at lower strike
        let short_call_low = Options::new(
            OptionType::European,
            Side::Short,
            underlying_symbol.clone(),
            low_strike,
            expiration.clone(),
            implied_volatility,
            quantity,
            underlying_price,
            risk_free_rate,
            OptionStyle::Call,
            dividend_yield,
            None,
        );
        strategy.short_call_low = Position::new(
            short_call_low,
            premium_low,
            Utc::now(),
            open_fee_short_call_low,
            close_fee_short_call_low,
        );

        // Create short call at higher strike
        let short_call_high = Options::new(
            OptionType::European,
            Side::Short,
            underlying_symbol,
            high_strike,
            expiration,
            implied_volatility,
            quantity,
            underlying_price,
            risk_free_rate,
            OptionStyle::Call,
            dividend_yield,
            None,
        );
        strategy.short_call_high = Position::new(
            short_call_high,
            premium_high,
            Utc::now(),
            open_fee_short_call_high,
            close_fee_short_call_high,
        );

        strategy.validate();

        let left_net_value = strategy
            .calculate_profit_at(strategy.short_call_low.option.strike_price)
            .unwrap()
            / quantity;
        let right_net_value = strategy
            .calculate_profit_at(strategy.short_call_high.option.strike_price)
            .unwrap()
            / quantity;

        if left_net_value >= Decimal::ZERO {
            strategy
                .break_even_points
                .push((strategy.short_call_low.option.strike_price + left_net_value).round_to(2));
        }

        if right_net_value >= Decimal::ZERO {
            strategy
                .break_even_points
                .push((strategy.short_call_high.option.strike_price - right_net_value).round_to(2));
        }

        strategy
    }
}

impl Validable for ShortButterflySpread {
    fn validate(&self) -> bool {
        if !self.short_call_low.validate() {
            debug!("Short call (low strike) is invalid");
            return false;
        }
        if !self.long_calls.validate() {
            debug!("Long calls (middle strike) are invalid");
            return false;
        }
        if !self.short_call_high.validate() {
            debug!("Short call (high strike) is invalid");
            return false;
        }

        if self.short_call_low.option.strike_price >= self.long_calls.option.strike_price {
            debug!("Low strike must be lower than middle strike");
            return false;
        }
        if self.long_calls.option.strike_price >= self.short_call_high.option.strike_price {
            debug!("Middle strike must be lower than high strike");
            return false;
        }

        if self.long_calls.option.quantity != self.short_call_low.option.quantity * 2.0 {
            debug!("Middle strike quantity must be double the wing quantities");
            return false;
        }
        if self.short_call_low.option.quantity != self.short_call_high.option.quantity {
            debug!("Wing quantities must be equal");
            return false;
        }

        true
    }
}

impl Positionable for ShortButterflySpread {
    fn add_position(&mut self, position: &Position) -> Result<(), PositionError> {
        match &position.option.side {
            Side::Short => {
                // long_calls should be inserted first
                if position.option.strike_price < self.long_calls.option.strike_price {
                    self.short_call_low = position.clone();
                    Ok(())
                } else {
                    self.short_call_high = position.clone();
                    Ok(())
                }
            }
            Side::Long => {
                self.long_calls = position.clone();
                Ok(())
            }
        }
    }

    fn get_positions(&self) -> Result<Vec<&Position>, PositionError> {
        Ok(vec![
            &self.short_call_low,
            &self.long_calls,
            &self.short_call_high,
        ])
    }
}

impl Strategies for ShortButterflySpread {
    fn get_underlying_price(&self) -> Positive {
        self.short_call_low.option.underlying_price
    }

    fn max_profit(&self) -> Result<Positive, StrategyError> {
        let left_profit = self.calculate_profit_at(self.short_call_low.option.strike_price)?;
        let right_profit = self.calculate_profit_at(self.short_call_high.option.strike_price)?;
        let max_profit = left_profit.max(right_profit);
        if max_profit > Decimal::ZERO {
            Ok(max_profit.into())
        } else {
            Err(StrategyError::ProfitLossError(
                ProfitLossErrorKind::MaxProfitError {
                    reason: "Max profit is negative".to_string(),
                },
            ))
        }
    }

    fn max_loss(&self) -> Result<Positive, StrategyError> {
        let loss = self.calculate_profit_at(self.long_calls.option.strike_price)?;
        if loss > Decimal::ZERO {
            Err(StrategyError::ProfitLossError(
                ProfitLossErrorKind::MaxLossError {
                    reason: "Max loss is negative".to_string(),
                },
            ))
        } else {
            Ok(loss.abs().into())
        }
    }

    fn profit_area(&self) -> Result<Decimal, StrategyError> {
        let break_even_points = self.get_break_even_points()?;
        let left_profit = self.calculate_profit_at(self.short_call_low.option.strike_price)?;
        let right_profit = self.calculate_profit_at(self.short_call_high.option.strike_price)?;

        let result = if break_even_points.len() == 2 {
            left_profit + right_profit
        } else {
            left_profit.max(right_profit)
        };
        Ok(result)
    }

    fn profit_ratio(&self) -> Result<Decimal, StrategyError> {
        let max_profit = self.max_profit().unwrap_or(Positive::ZERO);
        let max_loss = self.max_loss().unwrap_or(Positive::ZERO);
        match (max_profit, max_loss) {
            (value, _) if value == Positive::ZERO => Ok(Decimal::ZERO),
            (_, value) if value == Positive::ZERO => Ok(Decimal::MAX),
            _ => Ok((max_profit / max_loss * 100.0).into()),
        }
    }

    fn get_break_even_points(&self) -> Result<&Vec<Positive>, StrategyError> {
        Ok(&self.break_even_points)
    }
}

impl Optimizable for ShortButterflySpread {
    type Strategy = ShortButterflySpread;

    fn filter_combinations<'a>(
        &'a self,
        option_chain: &'a OptionChain,
        side: FindOptimalSide,
    ) -> impl Iterator<Item = OptionDataGroup<'a>> {
        let underlying_price = self.get_underlying_price();
        let strategy = self.clone();
        option_chain
            .get_triple_iter()
            // Filter out invalid combinations based on FindOptimalSide
            .filter(move |(short_low, long, short_high)| {
                short_low.is_valid_optimal_side(underlying_price, &side)
                    && long.is_valid_optimal_side(underlying_price, &side)
                    && short_high.is_valid_optimal_side(underlying_price, &side)
            })
            .filter(move |(short_low, long, short_high)| {
                short_low.strike_price < long.strike_price
                    && long.strike_price < short_high.strike_price
            })
            // Filter out options with invalid bid/ask prices
            .filter(|(short_low, long, short_high)| {
                short_low.call_bid.unwrap_or(Positive::ZERO) > Positive::ZERO
                    && long.call_ask.unwrap_or(Positive::ZERO) > Positive::ZERO
                    && short_high.call_bid.unwrap_or(Positive::ZERO) > Positive::ZERO
            })
            // Filter out options that don't meet strategy constraints
            .filter(move |(short_low, long, short_high)| {
                let legs = StrategyLegs::ThreeLegs {
                    first: short_low,
                    second: long,
                    third: short_high,
                };
                let strategy = strategy.create_strategy(option_chain, &legs);
                strategy.validate() && strategy.max_profit().is_ok() && strategy.max_loss().is_ok()
            })
            // Map to OptionDataGroup
            .map(move |(short_low, long, short_high)| {
                OptionDataGroup::Three(short_low, long, short_high)
            })
    }

    fn find_optimal(
        &mut self,
        option_chain: &OptionChain,
        side: FindOptimalSide,
        criteria: OptimizationCriteria,
    ) {
        let mut best_value = Decimal::MIN;
        let strategy_clone = self.clone();
        let options_iter = strategy_clone.filter_combinations(option_chain, side);

        for option_data_group in options_iter {
            // Unpack the OptionDataGroup into individual options
            let (short_low, long, short_high) = match option_data_group {
                OptionDataGroup::Three(first, second, third) => (first, second, third),
                _ => panic!("Invalid OptionDataGroup"),
            };

            let legs = StrategyLegs::ThreeLegs {
                first: short_low,
                second: long,
                third: short_high,
            };
            let strategy = self.create_strategy(option_chain, &legs);
            // Calculate the current value based on the optimization criteria
            let current_value = match criteria {
                OptimizationCriteria::Ratio => strategy.profit_ratio().unwrap(),
                OptimizationCriteria::Area => strategy.profit_area().unwrap(),
            };

            if current_value > best_value {
                // Update the best value and replace the current strategy
                info!("Found better value: {}", current_value);
                best_value = current_value;
                *self = strategy.clone();
            }
        }
    }

    fn create_strategy(&self, chain: &OptionChain, legs: &StrategyLegs) -> Self::Strategy {
        match legs {
            StrategyLegs::ThreeLegs {
                first: low_strike,
                second: middle_strike,
                third: high_strike,
            } => ShortButterflySpread::new(
                chain.symbol.clone(),
                chain.underlying_price,
                low_strike.strike_price,
                middle_strike.strike_price,
                high_strike.strike_price,
                self.short_call_low.option.expiration_date.clone(),
                middle_strike.implied_volatility.unwrap() / 100.0,
                self.short_call_low.option.risk_free_rate,
                self.short_call_low.option.dividend_yield,
                self.short_call_low.option.quantity,
                low_strike.call_bid.unwrap(),
                middle_strike.call_ask.unwrap(),
                high_strike.call_bid.unwrap(),
                self.long_calls.open_fee,
                self.long_calls.close_fee,
                self.short_call_low.open_fee,
                self.short_call_low.close_fee,
                self.short_call_high.open_fee,
                self.short_call_high.close_fee,
            ),
            _ => panic!("Invalid number of legs for Short Butterfly strategy"),
        }
    }
}

impl Profit for ShortButterflySpread {
    fn calculate_profit_at(&self, price: Positive) -> Result<Decimal, Box<dyn Error>> {
        let price = Some(&price);
        Ok(self.short_call_low.pnl_at_expiration(&price)?
            + self.long_calls.pnl_at_expiration(&price)?
            + self.short_call_high.pnl_at_expiration(&price)?)
    }
}

impl Graph for ShortButterflySpread {
    fn title(&self) -> String {
        let strategy_title = format!(
            "{:?} Strategy on {} Size {}:",
            self.kind,
            self.short_call_low.option.underlying_symbol,
            self.short_call_low.option.quantity
        );

        let leg_titles = [
            format!(
                "Short Call Low Strike: ${}",
                self.short_call_low.option.strike_price
            ),
            format!(
                "Long Calls Middle Strike: ${}",
                self.long_calls.option.strike_price
            ),
            format!(
                "Short Call High Strike: ${}",
                self.short_call_high.option.strike_price
            ),
            format!("Expire: {}", self.short_call_low.option.expiration_date),
        ];

        format!("{}\n\t{}", strategy_title, leg_titles.join("\n\t"))
    }

    fn get_vertical_lines(&self) -> Vec<ChartVerticalLine<f64, f64>> {
        vec![ChartVerticalLine {
            x_coordinate: self.short_call_low.option.underlying_price.to_f64(),
            y_range: (-50000.0, 50000.0),
            label: format!(
                "Current Price: {}",
                self.short_call_low.option.underlying_price
            ),
            label_offset: (5.0, 5.0),
            line_color: ORANGE,
            label_color: ORANGE,
            line_style: ShapeStyle::from(&ORANGE).stroke_width(2),
            font_size: 18,
        }]
    }

    fn get_points(&self) -> Vec<ChartPoint<(f64, f64)>> {
        let mut points = Vec::new();
        let max_loss = self.max_loss().unwrap_or(Positive::ZERO);

        let left_profit = self
            .calculate_profit_at(self.short_call_low.option.strike_price)
            .unwrap()
            .to_f64()
            .unwrap();
        let right_profit = self
            .calculate_profit_at(self.short_call_high.option.strike_price)
            .unwrap()
            .to_f64()
            .unwrap();

        // Break-even points
        points.extend(
            self.break_even_points
                .iter()
                .enumerate()
                .map(|(i, &price)| ChartPoint {
                    coordinates: (price.to_f64(), 0.0),
                    label: format!(
                        "Break Even {}: {:.2}",
                        if i == 0 { "Lower" } else { "Upper" },
                        price
                    ),
                    label_offset: LabelOffsetType::Relative(3.0, 3.0),
                    point_color: DARK_BLUE,
                    label_color: DARK_BLUE,
                    point_size: 5,
                    font_size: 18,
                }),
        );

        // Maximum loss point (at middle strike)
        points.push(ChartPoint {
            coordinates: (
                self.long_calls.option.strike_price.to_f64(),
                -max_loss.to_f64(),
            ),
            label: format!("Max Loss {:.2}", -max_loss.to_f64()),
            label_offset: LabelOffsetType::Relative(3.0, -3.0),
            point_color: RED,
            label_color: RED,
            point_size: 5,
            font_size: 18,
        });

        let left_color = if left_profit > ZERO { DARK_GREEN } else { RED };

        // Maximum profit points (at wing strikes)
        points.push(ChartPoint {
            coordinates: (
                self.short_call_low.option.strike_price.to_f64(),
                left_profit,
            ),
            label: format!("Left Profit {:.2}", left_profit),
            label_offset: LabelOffsetType::Relative(-30.0, 3.0),
            point_color: left_color,
            label_color: left_color,
            point_size: 5,
            font_size: 18,
        });

        let right_color = if right_profit > ZERO { DARK_GREEN } else { RED };

        points.push(ChartPoint {
            coordinates: (
                self.short_call_high.option.strike_price.to_f64(),
                right_profit,
            ),
            label: format!("Right Profit {:.2}", right_profit),
            label_offset: LabelOffsetType::Relative(3.0, 3.0),
            point_color: right_color,
            label_color: right_color,
            point_size: 5,
            font_size: 18,
        });

        // Current price point
        points.push(self.get_point_at_price(self.short_call_low.option.underlying_price));

        points
    }
}

impl ProbabilityAnalysis for ShortButterflySpread {
    fn get_expiration(&self) -> Result<ExpirationDate, ProbabilityError> {
        Ok(self.short_call_low.option.expiration_date.clone())
    }

    fn get_risk_free_rate(&self) -> Option<Decimal> {
        Some(self.short_call_low.option.risk_free_rate)
    }

    fn get_profit_ranges(&self) -> Result<Vec<ProfitLossRange>, ProbabilityError> {
        let mut ranges = Vec::new();
        let break_even_points = self.get_break_even_points()?;

        let (mean_volatility, std_dev) = mean_and_std(vec![
            self.short_call_low.option.implied_volatility,
            self.long_calls.option.implied_volatility,
            self.short_call_high.option.implied_volatility,
        ]);

        let volatility_adjustment = Some(VolatilityAdjustment {
            base_volatility: mean_volatility,
            std_dev_adjustment: std_dev,
        });

        let mut lower_profit_range = ProfitLossRange::new(
            Some(self.short_call_low.option.strike_price),
            Some(break_even_points[0]),
            pos!(self.max_profit()?.to_f64()),
        )?;

        lower_profit_range.calculate_probability(
            self.get_underlying_price(),
            volatility_adjustment.clone(),
            None,
            self.get_expiration()?,
            self.get_risk_free_rate(),
        )?;

        ranges.push(lower_profit_range);

        let mut upper_profit_range = ProfitLossRange::new(
            Some(break_even_points[1]),
            Some(self.short_call_high.option.strike_price),
            pos!(self.max_profit()?.to_f64()),
        )?;

        upper_profit_range.calculate_probability(
            self.get_underlying_price(),
            volatility_adjustment,
            None,
            self.get_expiration()?,
            self.get_risk_free_rate(),
        )?;

        ranges.push(upper_profit_range);

        Ok(ranges)
    }

    fn get_loss_ranges(&self) -> Result<Vec<ProfitLossRange>, ProbabilityError> {
        let break_even_points = self.get_break_even_points()?;

        let (mean_volatility, std_dev) = mean_and_std(vec![
            self.short_call_low.option.implied_volatility,
            self.long_calls.option.implied_volatility,
            self.short_call_high.option.implied_volatility,
        ]);

        let mut loss_range = ProfitLossRange::new(
            Some(break_even_points[0]),
            Some(break_even_points[1]),
            pos!(self.max_loss()?.to_f64()),
        )?;

        loss_range.calculate_probability(
            self.get_underlying_price(),
            Some(VolatilityAdjustment {
                base_volatility: mean_volatility,
                std_dev_adjustment: std_dev,
            }),
            None,
            self.get_expiration()?,
            self.get_risk_free_rate(),
        )?;

        Ok(vec![loss_range])
    }
}

impl Greeks for ShortButterflySpread {
    fn get_options(&self) -> Result<Vec<&Options>, GreeksError> {
        Ok(vec![
            &self.short_call_low.option,
            &self.long_calls.option,
            &self.short_call_high.option,
        ])
    }
}

impl DeltaNeutrality for ShortButterflySpread {
    fn calculate_net_delta(&self) -> DeltaInfo {
        let short_call_low_delta = self.short_call_low.option.delta();
        let short_call_high_delta = self.short_call_high.option.delta();
        let long_calls_delta = self.long_calls.option.delta();
        let threshold = DELTA_THRESHOLD;
        let s_cl_delta = short_call_low_delta.unwrap();
        let s_ch_delta = short_call_high_delta.unwrap();
        let l_c_delta = long_calls_delta.unwrap();
        let delta = s_cl_delta + s_ch_delta + l_c_delta;

        DeltaInfo {
            net_delta: delta,
            individual_deltas: vec![s_cl_delta, s_ch_delta, l_c_delta],
            is_neutral: (delta).abs() < threshold,
            underlying_price: self.short_call_low.option.underlying_price,
            neutrality_threshold: threshold,
        }
    }

    fn get_atm_strike(&self) -> Positive {
        self.short_call_low.option.underlying_price
    }

    fn generate_delta_reducing_adjustments(&self) -> Vec<DeltaAdjustment> {
        let net_delta = self.calculate_net_delta().net_delta;
        let delta_low = self.short_call_low.option.delta().unwrap();
        let delta_high = self.short_call_high.option.delta().unwrap();
        let qty_low = Positive((net_delta.abs() / delta_low).abs());
        let qty_high = Positive((net_delta.abs() / delta_high).abs());
        vec![
            DeltaAdjustment::SellOptions {
                quantity: qty_low * self.short_call_low.option.quantity,
                strike: self.short_call_low.option.strike_price,
                option_type: OptionStyle::Call,
            },
            DeltaAdjustment::SellOptions {
                quantity: qty_high * self.short_call_high.option.quantity,
                strike: self.short_call_high.option.strike_price,
                option_type: OptionStyle::Call,
            },
        ]
    }

    fn generate_delta_increasing_adjustments(&self) -> Vec<DeltaAdjustment> {
        let net_delta = self.calculate_net_delta().net_delta;
        let delta = self.long_calls.option.delta().unwrap();
        let qty = Positive((net_delta.abs() / delta).abs());

        vec![DeltaAdjustment::BuyOptions {
            quantity: qty * self.long_calls.option.quantity,
            strike: self.long_calls.option.strike_price,
            option_type: OptionStyle::Call,
        }]
    }
}

#[cfg(test)]
mod tests_long_butterfly_spread {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use rust_decimal_macros::dec;

    fn create_test_butterfly() -> LongButterflySpread {
        LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),                      // underlying_price
            pos!(90.0),                       // low_strike
            pos!(100.0),                      // middle_strike
            pos!(110.0),                      // high_strike
            ExpirationDate::Days(pos!(30.0)), // expiration
            pos!(0.2),                        // implied_volatility
            dec!(0.05),                       // risk_free_rate
            Positive::ZERO,                   // dividend_yield
            pos!(1.0),                        // quantity
            pos!(3.0),                        // premium_low
            Positive::TWO,                    // premium_middle
            Positive::ONE,                    // premium_high
            pos!(0.05),                       // open_fee_short_call
            pos!(0.05),                       // close_fee_short_call
            pos!(0.05),                       // open_fee_long_call_low
            pos!(0.05),                       // close_fee_long_call_low
            pos!(0.05),                       // open_fee_long_call_high
            pos!(0.05),                       // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_new_butterfly_basic_properties() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.name, "Long Butterfly");
        assert_eq!(butterfly.kind, StrategyType::LongButterflySpread);
        assert!(!butterfly.description.is_empty());
        assert!(butterfly.description.contains("long butterfly spread"));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_strikes() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.long_call_low.option.strike_price, pos!(90.0));
        assert_eq!(butterfly.short_calls.option.strike_price, pos!(100.0));
        assert_eq!(butterfly.long_call_high.option.strike_price, pos!(110.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_quantities() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.long_call_low.option.quantity, pos!(1.0));
        assert_eq!(butterfly.short_calls.option.quantity, pos!(2.0)); // Double quantity
        assert_eq!(butterfly.long_call_high.option.quantity, pos!(1.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_sides() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.long_call_low.option.side, Side::Long);
        assert_eq!(butterfly.short_calls.option.side, Side::Short);
        assert_eq!(butterfly.long_call_high.option.side, Side::Long);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_option_styles() {
        let butterfly = create_test_butterfly();

        assert_eq!(
            butterfly.long_call_low.option.option_style,
            OptionStyle::Call
        );
        assert_eq!(butterfly.short_calls.option.option_style, OptionStyle::Call);
        assert_eq!(
            butterfly.long_call_high.option.option_style,
            OptionStyle::Call
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_expiration_consistency() {
        let butterfly = create_test_butterfly();
        let expiration = ExpirationDate::Days(pos!(30.0));

        assert_eq!(
            format!("{:?}", butterfly.long_call_low.option.expiration_date),
            format!("{:?}", expiration)
        );
        assert_eq!(
            format!("{:?}", butterfly.short_calls.option.expiration_date),
            format!("{:?}", expiration)
        );
        assert_eq!(
            format!("{:?}", butterfly.long_call_high.option.expiration_date),
            format!("{:?}", expiration)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_fees_distribution() {
        let butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            Positive::ONE, // open_fee_short_call
            pos!(0.05),    // close_fee_short_call
            pos!(0.05),    // open_fee_long_call_low
            pos!(0.05),    // close_fee_long_call_low
            Positive::ONE, // open_fee_long_call_high
            pos!(0.05),    // close_fee_long_call_high
        );

        assert_eq!(butterfly.long_call_low.open_fee, 0.05); // fees / 3
        assert_eq!(butterfly.short_calls.open_fee, 1.0); // fees / 3
        assert_eq!(butterfly.long_call_high.open_fee, 1.0); // fees / 3
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_break_even_points() {
        let butterfly = create_test_butterfly();
        let break_even_points = butterfly.break_even_points;

        assert_eq!(break_even_points.len(), 2);
        assert!(break_even_points[0] > butterfly.long_call_low.option.strike_price);
        assert!(break_even_points[0] < butterfly.short_calls.option.strike_price);
        assert!(break_even_points[1] > butterfly.short_calls.option.strike_price);
        assert!(break_even_points[1] < butterfly.long_call_high.option.strike_price);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_with_different_quantities() {
        let butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(2.0), // quantity = 2
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );

        assert_eq!(butterfly.long_call_low.option.quantity, pos!(2.0));
        assert_eq!(butterfly.short_calls.option.quantity, pos!(4.0)); // 2 * 2
        assert_eq!(butterfly.long_call_high.option.quantity, pos!(2.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_with_symmetric_strikes() {
        let butterfly = create_test_butterfly();

        let lower_width =
            butterfly.short_calls.option.strike_price - butterfly.long_call_low.option.strike_price;
        let upper_width = butterfly.long_call_high.option.strike_price
            - butterfly.short_calls.option.strike_price;

        assert_eq!(lower_width, upper_width);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_with_equal_implied_volatility() {
        let butterfly = create_test_butterfly();

        assert_eq!(
            butterfly.long_call_low.option.implied_volatility,
            butterfly.short_calls.option.implied_volatility
        );
        assert_eq!(
            butterfly.short_calls.option.implied_volatility,
            butterfly.long_call_high.option.implied_volatility
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_with_invalid_premiums() {
        let check_profit = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::ONE,
            Positive::ONE,
            pos!(1.05),  // open_fee_short_call
            pos!(10.05), // close_fee_short_call
            pos!(1.05),  // open_fee_long_call_low
            pos!(0.05),  // close_fee_long_call_low
            pos!(1.05),  // open_fee_long_call_high
            pos!(0.05),  // close_fee_long_call_high
        );
        assert!(check_profit.max_profit().is_err());
    }
}

#[cfg(test)]
mod tests_short_butterfly_spread {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use rust_decimal_macros::dec;

    fn create_test_butterfly() -> ShortButterflySpread {
        ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),                      // underlying_price
            pos!(90.0),                       // low_strike
            pos!(100.0),                      // middle_strike
            pos!(110.0),                      // high_strike
            ExpirationDate::Days(pos!(30.0)), // expiration
            pos!(0.2),                        // implied_volatility
            dec!(0.05),                       // risk_free_rate
            Positive::ZERO,                   // dividend_yield
            pos!(1.0),                        // quantity
            pos!(10.0),                       // premium_low
            Positive::ONE,                    // premium_middle
            pos!(0.5),                        // premium_high
            pos!(0.05),                       // open_fee_short_call
            pos!(0.05),                       // close_fee_short_call
            pos!(0.05),                       // open_fee_long_call_low
            pos!(0.05),                       // close_fee_long_call_low
            pos!(0.05),                       // open_fee_long_call_high
            pos!(0.05),                       // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_new_butterfly_basic_properties() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.name, "Short Butterfly");
        assert_eq!(butterfly.kind, StrategyType::ShortButterflySpread);
        assert!(!butterfly.description.is_empty());
        assert!(butterfly.description.contains("short butterfly spread"));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_strikes() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.short_call_low.option.strike_price, pos!(90.0));
        assert_eq!(butterfly.long_calls.option.strike_price, pos!(100.0));
        assert_eq!(butterfly.short_call_high.option.strike_price, pos!(110.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_quantities() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.short_call_low.option.quantity, pos!(1.0));
        assert_eq!(butterfly.long_calls.option.quantity, pos!(2.0)); // Double quantity
        assert_eq!(butterfly.short_call_high.option.quantity, pos!(1.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_sides() {
        let butterfly = create_test_butterfly();

        assert_eq!(butterfly.short_call_low.option.side, Side::Short);
        assert_eq!(butterfly.long_calls.option.side, Side::Long);
        assert_eq!(butterfly.short_call_high.option.side, Side::Short);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_option_styles() {
        let butterfly = create_test_butterfly();

        assert_eq!(
            butterfly.short_call_low.option.option_style,
            OptionStyle::Call
        );
        assert_eq!(butterfly.long_calls.option.option_style, OptionStyle::Call);
        assert_eq!(
            butterfly.short_call_high.option.option_style,
            OptionStyle::Call
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_expiration_consistency() {
        let butterfly = create_test_butterfly();
        let expiration = ExpirationDate::Days(pos!(30.0));

        assert_eq!(
            format!("{:?}", butterfly.short_call_low.option.expiration_date),
            format!("{:?}", expiration)
        );
        assert_eq!(
            format!("{:?}", butterfly.long_calls.option.expiration_date),
            format!("{:?}", expiration)
        );
        assert_eq!(
            format!("{:?}", butterfly.short_call_high.option.expiration_date),
            format!("{:?}", expiration)
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_fees_distribution() {
        let butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            Positive::ONE, // open_fee_short_call
            pos!(0.05),    // close_fee_short_call
            Positive::ONE, // open_fee_long_call_low
            pos!(0.05),    // close_fee_long_call_low
            Positive::ONE, // open_fee_long_call_high
            pos!(0.05),    // close_fee_long_call_high
        );

        assert_eq!(butterfly.short_call_low.open_fee, 1.0); // fees / 3
        assert_eq!(butterfly.long_calls.open_fee, 1.0); // fees / 3
        assert_eq!(butterfly.short_call_high.open_fee, 1.0); // fees / 3
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_break_even_points() {
        let butterfly = create_test_butterfly();
        let break_even_points = butterfly.break_even_points;

        assert_eq!(break_even_points.len(), 2);
        assert!(break_even_points[0] > butterfly.short_call_low.option.strike_price);
        assert!(break_even_points[0] < butterfly.long_calls.option.strike_price);
        assert!(break_even_points[1] > butterfly.long_calls.option.strike_price);
        assert!(break_even_points[1] < butterfly.short_call_high.option.strike_price);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_with_different_quantities() {
        let butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(2.0),
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );

        assert_eq!(butterfly.short_call_low.option.quantity, pos!(2.0));
        assert_eq!(butterfly.long_calls.option.quantity, pos!(4.0)); // 2 * 2
        assert_eq!(butterfly.short_call_high.option.quantity, pos!(2.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_with_symmetric_strikes() {
        let butterfly = create_test_butterfly();

        let lower_width =
            butterfly.long_calls.option.strike_price - butterfly.short_call_low.option.strike_price;
        let upper_width = butterfly.short_call_high.option.strike_price
            - butterfly.long_calls.option.strike_price;

        assert_eq!(lower_width, upper_width);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_with_equal_implied_volatility() {
        let butterfly = create_test_butterfly();

        assert_eq!(
            butterfly.short_call_low.option.implied_volatility,
            butterfly.long_calls.option.implied_volatility
        );
        assert_eq!(
            butterfly.long_calls.option.implied_volatility,
            butterfly.short_call_high.option.implied_volatility
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_underlying_price_consistency() {
        let butterfly = create_test_butterfly();
        let underlying_price = pos!(100.0);

        assert_eq!(
            butterfly.short_call_low.option.underlying_price,
            underlying_price
        );
        assert_eq!(
            butterfly.long_calls.option.underlying_price,
            underlying_price
        );
        assert_eq!(
            butterfly.short_call_high.option.underlying_price,
            underlying_price
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]

    fn test_butterfly_with_invalid_premiums() {
        let max_loss = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(10.0),
            Positive::ONE,
            pos!(10.0),
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        assert!(max_loss.max_loss().is_err());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_butterfly_risk_free_rate_consistency() {
        let butterfly = create_test_butterfly();
        let risk_free_rate = dec!(0.05);

        assert_eq!(
            butterfly.short_call_low.option.risk_free_rate,
            risk_free_rate
        );
        assert_eq!(butterfly.long_calls.option.risk_free_rate, risk_free_rate);
        assert_eq!(
            butterfly.short_call_high.option.risk_free_rate,
            risk_free_rate
        );
    }
}

#[cfg(test)]
mod tests_long_butterfly_validation {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_valid_position(side: Side, strike_price: Positive, quantity: Positive) -> Position {
        Position::new(
            Options::new(
                OptionType::European,
                side,
                "TEST".to_string(),
                strike_price,
                ExpirationDate::Days(pos!(30.0)),
                pos!(0.2),
                quantity,
                pos!(100.0),
                dec!(0.05),
                OptionStyle::Call,
                Positive::ZERO,
                None,
            ),
            Positive::ONE,
            Utc::now(),
            Positive::ZERO,
            Positive::ZERO,
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_valid_long_butterfly() {
        let butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        assert!(butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_invalid_long_call_low() {
        let mut butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        butterfly.long_call_low = create_valid_position(Side::Long, pos!(90.0), Positive::ZERO);
        assert!(!butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_invalid_strike_order_low() {
        let butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(100.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        assert!(!butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_invalid_quantities() {
        let mut butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        butterfly.short_calls = create_valid_position(Side::Short, pos!(100.0), pos!(1.0));
        assert!(!butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_unequal_wing_quantities() {
        let mut butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        butterfly.long_call_high = create_valid_position(Side::Long, pos!(110.0), pos!(2.0));
        assert!(!butterfly.validate());
    }
}

#[cfg(test)]
mod tests_short_butterfly_validation {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_valid_position(side: Side, strike_price: Positive, quantity: Positive) -> Position {
        Position::new(
            Options::new(
                OptionType::European,
                side,
                "TEST".to_string(),
                strike_price,
                ExpirationDate::Days(pos!(30.0)),
                pos!(0.2),
                quantity,
                pos!(100.0),
                dec!(0.05),
                OptionStyle::Call,
                Positive::ZERO,
                None,
            ),
            Positive::ONE,
            Utc::now(),
            Positive::ZERO,
            Positive::ZERO,
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_valid_short_butterfly() {
        let butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        assert!(butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_invalid_short_call_low() {
        let mut butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        butterfly.short_call_low = create_valid_position(Side::Short, pos!(90.0), Positive::ZERO);
        assert!(!butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_invalid_strike_order_high() {
        let butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(90.0),
            pos!(100.0),
            pos!(100.0),
            pos!(100.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        assert!(!butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_invalid_middle_quantities() {
        let mut butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        butterfly.long_calls = create_valid_position(Side::Long, pos!(100.0), pos!(1.0));
        assert!(!butterfly.validate());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_unequal_wing_quantities_short() {
        let mut butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            Positive::ONE,
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        butterfly.short_call_high = create_valid_position(Side::Short, pos!(110.0), pos!(2.0));
        assert!(!butterfly.validate());
    }
}

#[cfg(test)]
mod tests_butterfly_strategies {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use num_traits::ToPrimitive;
    use rust_decimal_macros::dec;

    fn create_test_long() -> LongButterflySpread {
        LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0), // underlying_price
            pos!(90.0),  // low_strike
            pos!(100.0), // middle_strike
            pos!(110.0), // high_strike
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),      // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(1.0),      // quantity
            pos!(3.0),      // premium_low
            Positive::TWO,  // premium_middle
            Positive::ONE,  // premium_high
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    fn create_test_short() -> ShortButterflySpread {
        ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0), // underlying_price
            pos!(90.0),  // low_strike
            pos!(100.0), // middle_strike
            pos!(110.0), // high_strike
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),      // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(1.0),      // quantity
            pos!(10.0),     // premium_low
            pos!(5.0),      // premium_middle
            Positive::ONE,  // premium_high
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_underlying_price() {
        let long_butterfly = create_test_long();
        let short_butterfly = create_test_short();

        assert_eq!(long_butterfly.get_underlying_price(), pos!(100.0));
        assert_eq!(short_butterfly.get_underlying_price(), pos!(100.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_add_leg_long_butterfly() {
        let mut butterfly = create_test_long();
        let new_long = Position::new(
            Options::new(
                OptionType::European,
                Side::Long,
                "TEST".to_string(),
                pos!(85.0),
                ExpirationDate::Days(pos!(30.0)),
                pos!(0.2),
                pos!(1.0),
                pos!(100.0),
                dec!(0.05),
                OptionStyle::Call,
                Positive::ZERO,
                None,
            ),
            Positive::ONE,
            Utc::now(),
            Positive::ZERO,
            Positive::ZERO,
        );

        butterfly
            .add_position(&new_long.clone())
            .expect("Failed to add position");
        assert_eq!(butterfly.long_call_low.option.strike_price, pos!(85.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_add_leg_short_butterfly() {
        let mut butterfly = create_test_short();
        let new_short = Position::new(
            Options::new(
                OptionType::European,
                Side::Short,
                "TEST".to_string(),
                pos!(85.0),
                ExpirationDate::Days(pos!(30.0)),
                pos!(0.2),
                pos!(1.0),
                pos!(100.0),
                dec!(0.05),
                OptionStyle::Call,
                Positive::ZERO,
                None,
            ),
            Positive::ONE,
            Utc::now(),
            Positive::ZERO,
            Positive::ZERO,
        );

        butterfly
            .add_position(&new_short.clone())
            .expect("Failed to add position");
        assert_eq!(butterfly.short_call_low.option.strike_price, pos!(85.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_get_legs() {
        let long_butterfly = create_test_long();
        let short_butterfly = create_test_short();

        assert_eq!(long_butterfly.get_positions().unwrap().len(), 3);
        assert_eq!(short_butterfly.get_positions().unwrap().len(), 3);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_max_profit_long_butterfly() {
        let butterfly = create_test_long();
        let max_profit = butterfly.max_profit().unwrap().to_dec();
        // Max profit at middle strike
        let expected_profit = butterfly.calculate_profit_at(pos!(100.0)).unwrap();
        assert_eq!(max_profit, expected_profit);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_max_loss_long_butterfly() {
        let butterfly = create_test_long();
        let max_loss = butterfly.max_loss().unwrap().to_dec();
        // Max loss at wings
        let left_loss = butterfly.calculate_profit_at(pos!(90.0)).unwrap();
        let right_loss = butterfly.calculate_profit_at(pos!(110.0)).unwrap();
        assert_eq!(max_loss, left_loss.min(right_loss).abs());
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_max_loss_short_butterfly() {
        let butterfly = create_test_short();
        let max_loss = butterfly.max_loss().unwrap();
        // Max loss at middle strike
        let expected_loss = 9.4;
        assert_eq!(max_loss.to_f64(), expected_loss);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_total_cost() {
        let long_butterfly = create_test_long();
        let short_butterfly = create_test_short();

        assert!(long_butterfly.total_cost().unwrap() > Positive::ZERO);
        assert!(short_butterfly.total_cost().unwrap() > Positive::ZERO);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_fees() {
        let butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            Positive::ONE, // open_fee_short_call
            Positive::ONE, // close_fee_short_call
            Positive::ONE, // open_fee_long_call_low
            Positive::ONE, // close_fee_long_call_low
            Positive::ONE, // open_fee_long_call_high
            Positive::ONE, // close_fee_long_call_high
        );
        assert_eq!(butterfly.fees().unwrap().to_f64(), 8.0);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_fees_bis() {
        let butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(2.0),
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            Positive::ONE, // open_fee_short_call
            Positive::ONE, // close_fee_short_call
            Positive::ONE, // open_fee_long_call_low
            Positive::ONE, // close_fee_long_call_low
            Positive::ONE, // open_fee_long_call_high
            Positive::ONE, // close_fee_long_call_high
        );

        assert_eq!(butterfly.fees().unwrap(), pos!(16.0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_area_long_butterfly() {
        let butterfly = create_test_long();
        let area = butterfly.profit_area().unwrap().to_f64().unwrap();
        assert!(area > ZERO);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_area_short_butterfly() {
        let butterfly = create_test_short();
        let area = butterfly.profit_area().unwrap().to_f64().unwrap();
        assert!(area >= ZERO);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_ratio() {
        let long_butterfly = create_test_long();
        let short_butterfly = create_test_short();

        assert!(long_butterfly.profit_ratio().unwrap().to_f64().unwrap() > ZERO);
        assert!(short_butterfly.profit_ratio().unwrap().to_f64().unwrap() >= ZERO);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_break_even_points() {
        let long_butterfly = create_test_long();
        let short_butterfly = create_test_short();

        assert_eq!(long_butterfly.get_break_even_points().unwrap().len(), 2);
        assert_eq!(short_butterfly.get_break_even_points().unwrap().len(), 2);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profits_with_quantities() {
        let long_butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(2.0), // quantity = 2
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );

        let base_butterfly = create_test_long();
        assert_eq!(
            long_butterfly.max_profit().unwrap().to_f64(),
            base_butterfly.max_profit().unwrap().to_f64() * 2.0
        );
    }
}

#[cfg(test)]
mod tests_butterfly_optimizable {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use crate::spos;
    use rust_decimal_macros::dec;

    fn create_test_option_chain() -> OptionChain {
        let mut chain = OptionChain::new("TEST", pos!(100.0), "2024-12-31".to_string(), None, None);

        for strike in [85.0, 90.0, 95.0, 100.0, 105.0, 110.0, 115.0] {
            chain.add_option(
                pos!(strike),
                spos!(5.0),      // call_bid
                spos!(5.2),      // call_ask
                spos!(5.0),      // put_bid
                spos!(5.2),      // put_ask
                spos!(0.2),      // implied_volatility
                Some(dec!(0.5)), // delta
                spos!(100.0),    // volume
                Some(50),        // open_interest
            );
        }
        chain
    }

    fn create_test_long() -> LongButterflySpread {
        LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        )
    }

    fn create_test_short() -> ShortButterflySpread {
        ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            Positive::ONE,
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_find_optimal_area() {
        let mut butterfly = create_test_long();
        let chain = create_test_option_chain();
        let initial_area = butterfly.profit_area().unwrap();

        butterfly.find_optimal(&chain, FindOptimalSide::All, OptimizationCriteria::Area);

        assert!(butterfly.validate());
        assert!(butterfly.profit_area().unwrap() >= initial_area);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_valid_strike_order() {
        let mut butterfly = create_test_long();
        let chain = create_test_option_chain();

        butterfly.find_optimal(&chain, FindOptimalSide::All, OptimizationCriteria::Ratio);

        assert!(
            butterfly.long_call_low.option.strike_price < butterfly.short_calls.option.strike_price
        );
        assert!(
            butterfly.short_calls.option.strike_price
                < butterfly.long_call_high.option.strike_price
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_find_optimal_ratio_short() {
        let mut butterfly = create_test_short();
        let chain = create_test_option_chain();
        let initial_ratio = butterfly.profit_ratio().unwrap();

        butterfly.find_optimal(&chain, FindOptimalSide::All, OptimizationCriteria::Ratio);

        assert!(butterfly.validate());
        assert!(butterfly.profit_ratio().unwrap() >= initial_ratio);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_find_optimal_area_long() {
        let mut butterfly = create_test_long();
        let chain = create_test_option_chain();
        let initial_area = butterfly.profit_area().unwrap();

        butterfly.find_optimal(&chain, FindOptimalSide::All, OptimizationCriteria::Area);

        assert!(butterfly.validate());
        assert!(butterfly.profit_area().unwrap() >= initial_area);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_valid_strike_order_short() {
        let mut butterfly = create_test_short();
        let chain = create_test_option_chain();

        butterfly.find_optimal(&chain, FindOptimalSide::All, OptimizationCriteria::Ratio);

        assert!(
            butterfly.short_call_low.option.strike_price < butterfly.long_calls.option.strike_price
        );
        assert!(
            butterfly.long_calls.option.strike_price
                < butterfly.short_call_high.option.strike_price
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_find_optimal_with_range() {
        let mut long_butterfly = create_test_long();
        let mut short_butterfly = create_test_short();
        let chain = create_test_option_chain();

        long_butterfly.find_optimal(
            &chain,
            FindOptimalSide::Range(pos!(95.0), pos!(105.0)),
            OptimizationCriteria::Ratio,
        );
        short_butterfly.find_optimal(
            &chain,
            FindOptimalSide::Range(pos!(95.0), pos!(105.0)),
            OptimizationCriteria::Ratio,
        );

        assert!(long_butterfly.short_calls.option.strike_price >= pos!(95.0));
        assert!(long_butterfly.short_calls.option.strike_price <= pos!(105.0));
        assert!(short_butterfly.long_calls.option.strike_price >= pos!(95.0));
        assert!(short_butterfly.long_calls.option.strike_price <= pos!(105.0));
    }
}

#[cfg(test)]
mod tests_long_butterfly_profit {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use approx::assert_relative_eq;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    fn create_test() -> LongButterflySpread {
        LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0), // underlying_price
            pos!(90.0),  // low_strike
            pos!(100.0), // middle_strike
            pos!(110.0), // high_strike
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),      // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(1.0),      // quantity
            pos!(3.0),      // premium_low
            Positive::TWO,  // premium_middle
            Positive::ONE,  // premium_high
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_at_middle_strike() {
        let butterfly = create_test();
        let profit = butterfly.calculate_profit_at(pos!(100.0)).unwrap();
        assert!(profit > Decimal::ZERO);
        let expected = Positive::new_decimal(Decimal::from_str("9.6").unwrap()).unwrap();
        assert_eq!(profit, expected);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_below_lowest_strike() {
        let butterfly = create_test();
        let profit = butterfly
            .calculate_profit_at(pos!(85.0))
            .unwrap()
            .to_f64()
            .unwrap();
        assert!(profit < ZERO);
        let expected = 0.4;
        assert_relative_eq!(-profit, expected, epsilon = 0.0001);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_above_highest_strike() {
        let butterfly = create_test();
        let profit = butterfly.calculate_profit_at(pos!(115.0)).unwrap();
        assert!(profit < Decimal::ZERO);
        assert_relative_eq!(
            profit.to_f64().unwrap(),
            -butterfly.max_loss().unwrap().to_f64(),
            epsilon = 0.0001
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_at_break_even_points() {
        let butterfly = create_test();
        let break_even_points = butterfly.get_break_even_points().unwrap();

        for &point in break_even_points {
            let profit = butterfly
                .calculate_profit_at(point)
                .unwrap()
                .to_f64()
                .unwrap();
            assert_relative_eq!(profit, 0.0, epsilon = 0.01);
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_with_different_quantities() {
        let butterfly = LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0), // underlying_price
            pos!(90.0),  // low_strike
            pos!(100.0), // middle_strike
            pos!(110.0), // high_strike
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),      // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(2.0),      // quantity = 2
            pos!(3.0),      // premium_low
            Positive::TWO,  // premium_middle
            Positive::ONE,  // premium_high
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        );

        let scaled_profit = butterfly
            .calculate_profit_at(pos!(100.0))
            .unwrap()
            .to_f64()
            .unwrap();
        assert_relative_eq!(scaled_profit, 19.2, epsilon = 0.0001);
    }
}

#[cfg(test)]
mod tests_short_butterfly_profit {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use approx::assert_relative_eq;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    fn create_test() -> ShortButterflySpread {
        ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0), // underlying_price
            pos!(90.0),  // low_strike
            pos!(100.0), // middle_strike
            pos!(110.0), // high_strike
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),      // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(1.0),      // quantity
            pos!(3.0),      // premium_low
            Positive::TWO,  // premium_middle
            Positive::ONE,  // premium_high
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_at_middle_strike() {
        let butterfly = create_test();
        let profit = butterfly.calculate_profit_at(pos!(100.0)).unwrap();
        assert!(profit < Decimal::ZERO);
        let expected = Positive::new_decimal(Decimal::from_str("10.4").unwrap()).unwrap();

        assert_eq!(-profit, expected);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_at_break_even_points() {
        let butterfly = create_test();
        let break_even_points = butterfly.get_break_even_points().unwrap();

        for &point in break_even_points {
            let profit = butterfly
                .calculate_profit_at(point)
                .unwrap()
                .to_f64()
                .unwrap();
            assert_relative_eq!(profit, 0.0, epsilon = 0.01);
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_with_different_quantities() {
        let butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(2.0), // quantity = 2
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        );
        let scaled_profit = butterfly
            .calculate_profit_at(pos!(85.0))
            .unwrap()
            .to_f64()
            .unwrap();
        assert_relative_eq!(scaled_profit, -0.8, epsilon = 0.0001);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_symmetry() {
        let butterfly = create_test();
        let low_extreme_profit = butterfly
            .calculate_profit_at(pos!(85.0))
            .unwrap()
            .to_f64()
            .unwrap();
        let high_extreme_profit = butterfly
            .calculate_profit_at(pos!(115.0))
            .unwrap()
            .to_f64()
            .unwrap();

        assert_relative_eq!(low_extreme_profit, high_extreme_profit, epsilon = 0.01);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_profit_with_fees() {
        let butterfly = ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            Positive::ZERO, // open_fee_short_call
            Positive::ZERO, // close_fee_short_call
            Positive::ZERO, // open_fee_long_call_low
            Positive::ZERO, // close_fee_long_call_low
            Positive::ZERO, // open_fee_long_call_high
            Positive::ZERO, // close_fee_long_call_high
        );

        let base_butterfly = create_test();
        let profit_without_fees = butterfly
            .calculate_profit_at(pos!(85.0))
            .unwrap()
            .to_f64()
            .unwrap();
        let profit_with_fees = base_butterfly
            .calculate_profit_at(pos!(85.0))
            .unwrap()
            .to_f64()
            .unwrap();
        assert!(profit_with_fees < profit_without_fees);
    }
}

#[cfg(test)]
mod tests_long_butterfly_graph {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use rust_decimal_macros::dec;

    fn create_test_butterfly() -> LongButterflySpread {
        LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0), // underlying_price
            pos!(90.0),  // low_strike
            pos!(100.0), // middle_strike
            pos!(110.0), // high_strike
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            Positive::ONE,
            pos!(3.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_title_format() {
        let butterfly = create_test_butterfly();
        let title = butterfly.title();

        assert!(title.contains("LongButterflySpread Strategy"));
        assert!(title.contains("TEST"));
        assert!(title.contains("Size 1"));
        assert!(title.contains("Long Call Low Strike: $90"));
        assert!(title.contains("Short Calls Middle Strike: $100"));
        assert!(title.contains("Long Call High Strike: $110"));
        assert!(title.contains("Expire"));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_vertical_lines() {
        let butterfly = create_test_butterfly();
        let lines = butterfly.get_vertical_lines();

        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.x_coordinate, 100.0);
        assert_eq!(line.y_range, (-50000.0, 50000.0));
        assert!(line.label.contains("Current Price: 100"));
        assert_eq!(line.line_color, ORANGE);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_get_points() {
        let butterfly = create_test_butterfly();
        let points = butterfly.get_points();

        assert_eq!(points.len(), 6);

        let break_even_points: Vec<&ChartPoint<(f64, f64)>> = points
            .iter()
            .filter(|p| p.label.contains("Break Even"))
            .collect();
        assert_eq!(break_even_points.len(), 2);
        for point in break_even_points {
            assert_eq!(point.coordinates.1, 0.0);
            assert_eq!(point.point_color, DARK_BLUE);
        }

        let max_profit_point = points
            .iter()
            .find(|p| p.label.contains("Max Profit"))
            .unwrap();
        assert_eq!(max_profit_point.coordinates.0, 100.0);
        assert_eq!(max_profit_point.point_color, DARK_GREEN);

        let loss_points: Vec<&ChartPoint<(f64, f64)>> =
            points.iter().filter(|p| p.label.contains("Loss")).collect();
        for point in loss_points {
            assert!(point.coordinates.1 <= 0.0);
            assert_eq!(point.point_color, RED);
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_point_colors() {
        let butterfly = create_test_butterfly();
        let points = butterfly.get_points();

        for point in points {
            match point.label.as_str() {
                label if label.contains("Break Even") => {
                    assert_eq!(point.point_color, DARK_BLUE);
                    assert_eq!(point.label_color, DARK_BLUE);
                }
                label if label.contains("Max Profit") => {
                    assert_eq!(point.point_color, DARK_GREEN);
                    assert_eq!(point.label_color, DARK_GREEN);
                }
                label if label.contains("Loss") && point.coordinates.1 < 0.0 => {
                    assert_eq!(point.point_color, RED);
                    assert_eq!(point.label_color, RED);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests_short_butterfly_graph {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use approx::assert_relative_eq;
    use rust_decimal_macros::dec;

    fn create_test_butterfly() -> ShortButterflySpread {
        let underlying_price = pos!(5781.88);
        ShortButterflySpread::new(
            "SP500".to_string(),
            underlying_price, // underlying_price
            pos!(5700.0),     // short_strike_itm
            pos!(5780.0),     // long_strike
            pos!(5850.0),     // short_strike_otm
            ExpirationDate::Days(pos!(2.0)),
            pos!(0.18),     // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(1.0),      // long quantity
            pos!(119.01),   // premium_long
            pos!(66.0),     // premium_short
            pos!(29.85),    // open_fee_long
            pos!(0.05),
            pos!(0.05),
            pos!(0.05),
            pos!(0.05),
            pos!(0.05),
            pos!(0.05),
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_title_format() {
        let butterfly = create_test_butterfly();
        let title = butterfly.title();

        assert!(title.contains("ShortButterflySpread Strategy"));
        assert!(title.contains("Size 1"));
        assert!(title.contains("Short Call Low Strike: $5700"));
        assert!(title.contains("Long Calls Middle Strike: $5780"));
        assert!(title.contains("Short Call High Strike: $5850"));
        assert!(title.contains("Expire"));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_vertical_lines() {
        let butterfly = create_test_butterfly();
        let lines = butterfly.get_vertical_lines();

        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.x_coordinate, 5781.88);
        assert_eq!(line.y_range, (-50000.0, 50000.0));
        assert!(line.label.contains("Current Price: 5781.88"));
        assert_eq!(line.line_color, ORANGE);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_get_points() {
        let butterfly = create_test_butterfly();
        let points = butterfly.get_points();

        assert_eq!(points.len(), 6);

        let break_even_points: Vec<&ChartPoint<(f64, f64)>> = points
            .iter()
            .filter(|p| p.label.contains("Break Even"))
            .collect();
        assert_eq!(break_even_points.len(), 2);
        for point in break_even_points {
            assert_eq!(point.coordinates.1, 0.0);
            assert_eq!(point.point_color, DARK_BLUE);
        }

        let max_loss_point = points
            .iter()
            .find(|p| p.label.contains("Max Loss"))
            .unwrap();
        assert_eq!(max_loss_point.coordinates.0, 5780.0);
        assert_eq!(max_loss_point.point_color, RED);

        let profit_points: Vec<&ChartPoint<(f64, f64)>> = points
            .iter()
            .filter(|p| p.label.contains("Profit"))
            .collect();
        for point in profit_points {
            assert!(point.coordinates.1 >= 0.0);
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_point_symmetry() {
        let butterfly = create_test_butterfly();
        let points = butterfly.get_points();

        let profit_points: Vec<&ChartPoint<(f64, f64)>> = points
            .iter()
            .filter(|p| p.label.contains("Profit"))
            .collect();

        assert_relative_eq!(profit_points[0].coordinates.1, 16.46, epsilon = 0.01);
        assert_relative_eq!(profit_points[1].coordinates.1, 6.46, epsilon = 0.01);
    }
}

#[cfg(test)]
mod tests_butterfly_probability {
    use super::*;
    use crate::model::types::ExpirationDate;
    use crate::pos;
    use rust_decimal_macros::dec;

    fn create_test_long() -> LongButterflySpread {
        LongButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(10.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        )
    }

    fn create_test_short() -> ShortButterflySpread {
        ShortButterflySpread::new(
            "TEST".to_string(),
            pos!(100.0),
            pos!(90.0),
            pos!(100.0),
            pos!(110.0),
            ExpirationDate::Days(pos!(30.0)),
            pos!(0.2),
            dec!(0.05),
            Positive::ZERO,
            pos!(1.0),
            pos!(10.0),
            Positive::TWO,
            Positive::ONE,
            pos!(0.05), // open_fee_short_call
            pos!(0.05), // close_fee_short_call
            pos!(0.05), // open_fee_long_call_low
            pos!(0.05), // close_fee_long_call_low
            pos!(0.05), // open_fee_long_call_high
            pos!(0.05), // close_fee_long_call_high
        )
    }

    mod long_butterfly_tests {
        use super::*;

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_expiration() {
            let butterfly = create_test_long();
            let expiration = butterfly.get_expiration().unwrap();
            match expiration {
                ExpirationDate::Days(days) => assert_eq!(days, 30.0),
                _ => panic!("Expected ExpirationDate::Days"),
            }
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_risk_free_rate() {
            let butterfly = create_test_long();
            assert_eq!(butterfly.get_risk_free_rate(), Some(dec!(0.05)));
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_profit_ranges() {
            let butterfly = create_test_long();
            let ranges = butterfly.get_profit_ranges().unwrap();

            assert_eq!(ranges.len(), 1);
            let range = &ranges[0];

            let break_even_points = butterfly.get_break_even_points().unwrap();
            assert_eq!(range.lower_bound.unwrap(), break_even_points[0]);
            assert_eq!(range.upper_bound.unwrap(), break_even_points[1]);
            assert!(range.probability > Positive::ZERO);
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_loss_ranges() {
            let butterfly = create_test_long();
            let ranges = butterfly.get_loss_ranges().unwrap();

            assert_eq!(ranges.len(), 2);

            let lower_range = &ranges[0];
            assert_eq!(
                lower_range.lower_bound.unwrap(),
                butterfly.long_call_low.option.strike_price
            );
            assert_eq!(
                lower_range.upper_bound.unwrap(),
                butterfly.get_break_even_points().unwrap()[0]
            );
            assert!(lower_range.probability > Positive::ZERO);

            let upper_range = &ranges[1];
            assert_eq!(
                upper_range.lower_bound.unwrap(),
                butterfly.get_break_even_points().unwrap()[1]
            );
            assert_eq!(
                upper_range.upper_bound.unwrap(),
                butterfly.long_call_high.option.strike_price
            );
            assert!(upper_range.probability > Positive::ZERO);
        }
    }

    mod short_butterfly_tests {
        use super::*;

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_expiration() {
            let butterfly = create_test_short();
            let expiration = butterfly.get_expiration().unwrap();
            match expiration {
                ExpirationDate::Days(days) => assert_eq!(days, 30.0),
                _ => panic!("Expected ExpirationDate::Days"),
            }
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_risk_free_rate() {
            let butterfly = create_test_short();
            assert_eq!(butterfly.get_risk_free_rate(), Some(dec!(0.05)));
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_profit_ranges() {
            let butterfly = create_test_short();
            let ranges = butterfly.get_profit_ranges().unwrap();

            assert_eq!(ranges.len(), 2);

            let lower_range = &ranges[0];
            assert_eq!(
                lower_range.lower_bound.unwrap(),
                butterfly.short_call_low.option.strike_price
            );
            assert_eq!(
                lower_range.upper_bound.unwrap(),
                butterfly.get_break_even_points().unwrap()[0]
            );
            assert!(lower_range.probability > Positive::ZERO);

            let upper_range = &ranges[1];
            assert_eq!(
                upper_range.lower_bound.unwrap(),
                butterfly.get_break_even_points().unwrap()[1]
            );
            assert_eq!(
                upper_range.upper_bound.unwrap(),
                butterfly.short_call_high.option.strike_price
            );
            assert!(upper_range.probability > Positive::ZERO);
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_get_loss_ranges() {
            let butterfly = create_test_short();
            let ranges = butterfly.get_loss_ranges().unwrap();

            assert_eq!(ranges.len(), 1);
            let range = &ranges[0];

            let break_even_points = butterfly.get_break_even_points().unwrap();
            assert_eq!(range.lower_bound.unwrap(), break_even_points[0]);
            assert_eq!(range.upper_bound.unwrap(), break_even_points[1]);
            assert!(range.probability > Positive::ZERO);
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_volatility_calculations() {
        let long_butterfly = create_test_long();
        let short_butterfly = create_test_short();

        let long_ranges = long_butterfly.get_profit_ranges().unwrap();
        let short_ranges = short_butterfly.get_profit_ranges().unwrap();

        assert!(!long_ranges.is_empty());
        assert!(!short_ranges.is_empty());
        assert!(long_ranges[0].probability > Positive::ZERO);
        assert!(short_ranges[0].probability > Positive::ZERO);
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_probability_sum() {
        let long_butterfly = create_test_long();
        let short_butterfly = create_test_short();

        let long_profit_ranges = long_butterfly.get_profit_ranges().unwrap();
        let long_loss_ranges = long_butterfly.get_loss_ranges().unwrap();
        let long_total_prob = long_profit_ranges
            .iter()
            .map(|r| r.probability.to_f64())
            .sum::<f64>()
            + long_loss_ranges
                .iter()
                .map(|r| r.probability.to_f64())
                .sum::<f64>();
        assert!((long_total_prob - 1.0).abs() < 0.1);

        let short_profit_ranges = short_butterfly.get_profit_ranges().unwrap();
        let short_loss_ranges = short_butterfly.get_loss_ranges().unwrap();
        let short_total_prob = short_profit_ranges
            .iter()
            .map(|r| r.probability.to_f64())
            .sum::<f64>()
            + short_loss_ranges
                .iter()
                .map(|r| r.probability.to_f64())
                .sum::<f64>();
        assert!((short_total_prob - 1.0).abs() < 0.1);
    }
}

#[cfg(test)]
mod tests_long_butterfly_delta {
    use super::*;
    use crate::model::types::{ExpirationDate, OptionStyle};
    use crate::strategies::butterfly_spread::LongButterflySpread;
    use crate::strategies::delta_neutral::DELTA_THRESHOLD;
    use crate::strategies::delta_neutral::{DeltaAdjustment, DeltaNeutrality};
    use crate::{assert_decimal_eq, assert_pos_relative_eq, pos};

    use rust_decimal_macros::dec;

    fn get_strategy(underlying_price: Positive) -> LongButterflySpread {
        LongButterflySpread::new(
            "SP500".to_string(),
            underlying_price, // underlying_price
            pos!(5710.0),     // long_strike_itm
            pos!(5820.0),     // short_strike
            pos!(6100.0),     // long_strike_otm
            ExpirationDate::Days(pos!(2.0)),
            pos!(0.18),     // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(1.0),      // long quantity
            pos!(49.65),    // premium_long
            pos!(42.93),    // premium_short
            Positive::ONE,  // open_fee_long
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_reducing_adjustments() {
        let strategy = get_strategy(pos!(5881.88));
        let size = dec!(-0.5970615569);
        let delta1 = pos!(0.60439151471911);
        let delta2 = pos!(175.125_739_348_840_2);
        let k1 = pos!(5710.0);
        let k2 = pos!(6100.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion_zero = binding.first().unwrap();
        let suggestion_one = binding.last().unwrap();
        match suggestion_zero {
            DeltaAdjustment::BuyOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta1, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k1, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        match suggestion_one {
            DeltaAdjustment::BuyOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta2, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k2, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.long_call_low.option.clone();
        option.quantity = delta1;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_increasing_adjustments() {
        let strategy = get_strategy(pos!(5710.81));
        let size = dec!(0.3518);
        let delta = pos!(4.310_394_079_825_43);
        let k = pos!(5820.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion = binding.first().unwrap();
        match suggestion {
            DeltaAdjustment::SellOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.short_calls.option.clone();
        option.quantity = delta;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_no_adjustments() {
        let strategy = get_strategy(pos!(5420.0));

        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
        assert!(strategy.is_delta_neutral());
        let suggestion = strategy.suggest_delta_adjustments();
        assert_eq!(suggestion[0], DeltaAdjustment::NoAdjustmentNeeded);
    }
}

#[cfg(test)]
mod tests_long_butterfly_delta_size {
    use super::*;
    use crate::model::types::{ExpirationDate, OptionStyle};
    use crate::strategies::butterfly_spread::LongButterflySpread;
    use crate::strategies::delta_neutral::DELTA_THRESHOLD;
    use crate::strategies::delta_neutral::{DeltaAdjustment, DeltaNeutrality};
    use crate::{assert_decimal_eq, assert_pos_relative_eq, pos};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    fn get_strategy(underlying_price: Positive) -> LongButterflySpread {
        LongButterflySpread::new(
            "SP500".to_string(),
            underlying_price, // underlying_price
            pos!(5710.0),     // long_strike_itm
            pos!(5820.0),     // short_strike
            pos!(6100.0),     // long_strike_otm
            ExpirationDate::Days(pos!(2.0)),
            pos!(0.18),     // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(3.0),      // long quantity
            pos!(49.65),    // premium_long
            pos!(42.93),    // premium_short
            Positive::ONE,  // open_fee_long
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_reducing_adjustments() {
        let strategy = get_strategy(pos!(5881.85));
        let size = dec!(-1.7905);
        let delta1 = pos!(1.812583011030011);
        let delta2 = pos!(525.8051045358664);
        let k1 = pos!(5710.0);
        let k2 = pos!(6100.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion_zero = binding.first().unwrap();
        let suggestion_one = binding.last().unwrap();
        match suggestion_zero {
            DeltaAdjustment::BuyOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta1, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k1, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        match suggestion_one {
            DeltaAdjustment::BuyOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta2, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k2, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.long_call_low.option.clone();
        option.quantity = delta1;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_increasing_adjustments() {
        let strategy = get_strategy(pos!(5710.88));
        let size = dec!(1.0558);
        let delta =
            Positive::new_decimal(Decimal::from_str("12.912467384337744").unwrap()).unwrap();
        let k = pos!(5820.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion = binding.first().unwrap();
        match suggestion {
            DeltaAdjustment::SellOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.short_calls.option.clone();
        option.quantity = delta;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_no_adjustments() {
        let strategy = get_strategy(pos!(5410.0));

        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
        assert!(strategy.is_delta_neutral());
        let suggestion = strategy.suggest_delta_adjustments();
        assert_eq!(suggestion[0], DeltaAdjustment::NoAdjustmentNeeded);
    }
}

#[cfg(test)]
mod tests_short_butterfly_delta {
    use super::*;
    use crate::model::types::{ExpirationDate, OptionStyle};
    use crate::strategies::butterfly_spread::ShortButterflySpread;
    use crate::strategies::delta_neutral::DELTA_THRESHOLD;
    use crate::strategies::delta_neutral::{DeltaAdjustment, DeltaNeutrality};
    use crate::{assert_decimal_eq, assert_pos_relative_eq, pos};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    fn get_strategy(underlying_price: Positive) -> ShortButterflySpread {
        ShortButterflySpread::new(
            "SP500".to_string(),
            underlying_price, // underlying_price
            pos!(5700.0),     // short_strike_itm
            pos!(5780.0),     // long_strike
            pos!(5850.0),     // short_strike_otm
            ExpirationDate::Days(pos!(2.0)),
            pos!(0.18),     // implied_volatility
            Decimal::ZERO,  // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(1.0),      // long quantity
            pos!(119.01),   // premium_long
            pos!(66.0),     // premium_short
            pos!(29.85),    // open_fee_long
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_reducing_adjustments() {
        let strategy = get_strategy(pos!(5781.88));
        let size = dec!(-0.0259);
        let delta =
            Positive::new_decimal(Decimal::from_str("0.05072646985065364").unwrap()).unwrap();
        let k = pos!(5780.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion = binding.first().unwrap();
        match suggestion {
            DeltaAdjustment::BuyOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.long_calls.option.clone();
        option.quantity = delta;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_increasing_adjustments() {
        let strategy = get_strategy(pos!(5881.88));
        let size = dec!(0.16077612);
        let delta1 = pos!(0.16224251196539);
        let delta2 = pos!(0.24331842376268);
        let k1 = pos!(5700.0);
        let k2 = pos!(5850.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion_zero = binding.first().unwrap();
        let suggestion_one = binding.last().unwrap();
        match suggestion_zero {
            DeltaAdjustment::SellOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta1, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k1, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        match suggestion_one {
            DeltaAdjustment::SellOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta2, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k2, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.short_call_low.option.clone();
        option.quantity = delta1;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_no_adjustments() {
        let strategy = get_strategy(pos!(5788.55));

        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
        assert!(strategy.is_delta_neutral());
        let suggestion = strategy.suggest_delta_adjustments();
        assert_eq!(suggestion[0], DeltaAdjustment::NoAdjustmentNeeded);
    }
}

#[cfg(test)]
mod tests_short_butterfly_delta_size {
    use super::*;
    use crate::model::types::{ExpirationDate, OptionStyle};
    use crate::strategies::butterfly_spread::ShortButterflySpread;
    use crate::strategies::delta_neutral::DELTA_THRESHOLD;
    use crate::strategies::delta_neutral::{DeltaAdjustment, DeltaNeutrality};
    use crate::{assert_decimal_eq, assert_pos_relative_eq, pos};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn get_strategy(underlying_price: Positive) -> ShortButterflySpread {
        ShortButterflySpread::new(
            "SP500".to_string(),
            underlying_price, // underlying_price
            pos!(5700.0),     // short_strike_itm
            pos!(5780.0),     // long_strike
            pos!(5850.0),     // short_strike_otm
            ExpirationDate::Days(pos!(2.0)),
            pos!(0.18),     // implied_volatility
            dec!(0.05),     // risk_free_rate
            Positive::ZERO, // dividend_yield
            pos!(3.0),      // long quantity
            pos!(119.01),   // premium_long
            pos!(66.0),     // premium_short
            pos!(29.85),    // open_fee_long
            pos!(0.05),     // open_fee_short_call
            pos!(0.05),     // close_fee_short_call
            pos!(0.05),     // open_fee_long_call_low
            pos!(0.05),     // close_fee_long_call_low
            pos!(0.05),     // open_fee_long_call_high
            pos!(0.05),     // close_fee_long_call_high
        )
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_reducing_adjustments() {
        let strategy = get_strategy(pos!(5781.88));
        let size = dec!(-0.0593);
        let delta = pos!(0.11409430831966512);
        let k = pos!(5780.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion = binding.first().unwrap();
        match suggestion {
            DeltaAdjustment::BuyOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.long_calls.option.clone();
        option.quantity = delta;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_increasing_adjustments() {
        let strategy = get_strategy(pos!(5881.88));
        let size = dec!(0.4787);
        let delta1 = pos!(0.4828726371186378);
        let delta2 = pos!(0.7164055343340598);
        let k1 = pos!(5700.0);
        let k2 = pos!(5850.0);
        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            size,
            DELTA_THRESHOLD
        );
        assert!(!strategy.is_delta_neutral());
        let binding = strategy.suggest_delta_adjustments();
        let suggestion_zero = binding.first().unwrap();
        let suggestion_one = binding.last().unwrap();
        match suggestion_zero {
            DeltaAdjustment::SellOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta1, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k1, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        match suggestion_one {
            DeltaAdjustment::SellOptions {
                quantity,
                strike,
                option_type,
            } => {
                assert_pos_relative_eq!(*quantity, delta2, Positive(DELTA_THRESHOLD));
                assert_pos_relative_eq!(*strike, k2, Positive(DELTA_THRESHOLD));
                assert_eq!(*option_type, OptionStyle::Call);
            }
            _ => panic!("Invalid suggestion"),
        }

        let mut option = strategy.short_call_low.option.clone();
        option.quantity = delta1;
        let delta = option.delta().unwrap();
        assert_decimal_eq!(delta, -size, DELTA_THRESHOLD);
        assert_decimal_eq!(
            delta + strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn create_test_no_adjustments() {
        let strategy = get_strategy(pos!(5786.99));

        assert_decimal_eq!(
            strategy.calculate_net_delta().net_delta,
            Decimal::ZERO,
            DELTA_THRESHOLD
        );
        assert!(strategy.is_delta_neutral());
        let suggestion = strategy.suggest_delta_adjustments();
        assert_eq!(suggestion[0], DeltaAdjustment::NoAdjustmentNeeded);
    }
}
