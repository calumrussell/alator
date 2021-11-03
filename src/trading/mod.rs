use std::collections::HashMap;

use time::OffsetDateTime;

pub trait TradingSystem {
    fn calculate_weights(&mut self) -> HashMap<String, f64>;
    fn should_trade_now(&self, date: &i64) -> bool;
}

pub trait TradingSchedule {
    fn should_trade(date: &i64) -> bool;
}

pub struct DefaultTradingSchedule;

impl TradingSchedule for DefaultTradingSchedule {
    fn should_trade(_date: &i64) -> bool {
        true
    }
}

pub struct LastBusinessDayTradingSchedule;

impl TradingSchedule for LastBusinessDayTradingSchedule {
    fn should_trade(date: &i64) -> bool {
        let time = OffsetDateTime::from_unix_timestamp(date.clone());

        let seconds_in_day = 86400;
        match time {
            Ok(t) => {
                if t.day() < (28 - 7) {
                    //Cannot be greater than a week before minimum number of days in month
                    return false;
                }

                match t.weekday() {
                    //Shouldn't pass anything but weekday but to be safe
                    time::Weekday::Saturday | time::Weekday::Sunday => return false,
                    _ => (),
                }

                /*
                Only need to go up to four as we are checking for weekends.
                The day offset_time should either be a weekend or a day with a different
                month, if either of these things is false then we return false.
                The day of the new month is not necessarily the first day.
                 */
                for i in 1..4 {
                    let offset_time =
                        OffsetDateTime::from_unix_timestamp(date + (i * seconds_in_day)).unwrap();
                    match offset_time.weekday() {
                        time::Weekday::Saturday | time::Weekday::Sunday => continue,
                        _ => {
                            if offset_time.month() == t.month() {
                                return false;
                            } else {
                                continue;
                            }
                        }
                    }
                }
                return true;
            }
            _ => {
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{LastBusinessDayTradingSchedule, TradingSchedule};

    #[test]
    fn test_that_schedule_returns_true_for_last_day_of_month() {
        // Date 30/09/21 - 17:00:0000
        assert!(LastBusinessDayTradingSchedule::should_trade(&1633021200));
        // Date 29/10/21 - 17:00:0000
        assert!(LastBusinessDayTradingSchedule::should_trade(&1635526800));
    }

    #[test]
    fn test_that_schedule_returns_false_for_non_last_day_of_month() {
        // Date 1/11/21 - 9:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(&1635757200));
        // Date 12/11/21 - 17:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(&1636736400));
        //Date 31/10/21 - 9:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(&1635670800));
        //Date 22/1/21 - 9:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(&1611306000));
    }
}
