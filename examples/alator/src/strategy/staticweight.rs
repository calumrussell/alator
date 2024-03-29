use log::info;
use rotala::exchange::uist::UistTrade;

use crate::broker::uist::UistBroker;
use crate::broker::{BrokerCashEvent, BrokerOperations, CashOperations, Portfolio, SendOrder};
use crate::perf::{BacktestOutput, PerformanceCalculator};
use crate::schedule::{DefaultTradingSchedule, TradingSchedule};
use crate::strategy::{Audit, History, Strategy, StrategyEvent, TransferFrom, TransferTo};
use crate::types::{CashValue, PortfolioAllocation, StrategySnapshot};
use rotala::clock::{Clock, Frequency};

pub struct StaticWeightStrategyBuilder {
    //If missing either field, we cannot run this strategy
    brkr: Option<UistBroker>,
    weights: Option<PortfolioAllocation>,
    clock: Option<Clock>,
}

impl StaticWeightStrategyBuilder {
    pub fn default(&mut self) -> StaticWeightStrategy {
        if self.brkr.is_none() || self.weights.is_none() || self.clock.is_none() {
            panic!("Strategy must have broker, weights, and clock");
        }

        let brkr = self.brkr.take();
        let weights = self.weights.take();
        StaticWeightStrategy {
            brkr: brkr.unwrap(),
            target_weights: weights.unwrap(),
            net_cash_flow: 0.0.into(),
            clock: self.clock.as_ref().unwrap().clone(),
            history: Vec::new(),
        }
    }

    pub fn with_clock(&mut self, clock: Clock) -> &mut Self {
        self.clock = Some(clock);
        self
    }

    pub fn with_brkr(&mut self, brkr: UistBroker) -> &mut Self {
        self.brkr = Some(brkr);
        self
    }

    pub fn with_weights(&mut self, weights: PortfolioAllocation) -> &mut Self {
        self.weights = Some(weights);
        self
    }

    pub fn new() -> Self {
        Self {
            brkr: None,
            weights: None,
            clock: None,
        }
    }
}

impl Default for StaticWeightStrategyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

///Basic implementation of an investment strategy which takes a set of fixed-weight allocations and
///rebalances over time towards those weights.
pub struct StaticWeightStrategy {
    brkr: UistBroker,
    target_weights: PortfolioAllocation,
    net_cash_flow: CashValue,
    clock: Clock,
    history: Vec<StrategySnapshot>,
}

impl StaticWeightStrategy {
    pub fn run(&mut self) {
        while self.clock.has_next() {
            self.update();
        }
    }

    pub fn perf(&self, freq: Frequency) -> BacktestOutput {
        //Intended to be called at end of simulation
        let hist = self.get_history();
        PerformanceCalculator::calculate(freq, hist)
    }

    pub fn get_snapshot(&mut self) -> StrategySnapshot {
        // Defaults to zero inflation because most users probably aren't looking
        // for real returns calcs
        let now = self.clock.now();
        StrategySnapshot {
            date: now,
            portfolio_value: self.brkr.get_total_value(),
            net_cash_flow: self.net_cash_flow.clone(),
            inflation: 0.0,
        }
    }
}

impl Strategy for StaticWeightStrategy {
    fn init(&mut self, initital_cash: &f64) {
        self.deposit_cash(initital_cash);
        if DefaultTradingSchedule::should_trade(&self.clock.now()) {
            let orders = self
                .brkr
                .diff_brkr_against_target_weights(&self.target_weights);
            if !orders.is_empty() {
                self.brkr.send_orders(&orders);
            }
        }
    }

    fn update(&mut self) {
        self.brkr.check();
        let now = self.clock.now();
        if DefaultTradingSchedule::should_trade(&now) {
            let orders = self
                .brkr
                .diff_brkr_against_target_weights(&self.target_weights);
            if !orders.is_empty() {
                self.brkr.send_orders(&orders);
            }
        }
        let snap = self.get_snapshot();
        self.history.push(snap);
    }
}

impl TransferTo for StaticWeightStrategy {
    fn deposit_cash(&mut self, cash: &f64) -> StrategyEvent {
        info!("STRATEGY: Depositing {:?} into strategy", cash);
        self.brkr.deposit_cash(cash);
        self.net_cash_flow = CashValue::from(cash + *self.net_cash_flow);
        StrategyEvent::DepositSuccess(CashValue::from(*cash))
    }
}

impl TransferFrom for StaticWeightStrategy {
    fn withdraw_cash(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) = self.brkr.withdraw_cash(cash) {
            info!("STRATEGY: Succesfully withdrew {:?} from strategy", cash);
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        info!("STRATEGY: Failed to withdraw {:?} from strategy", cash);
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }

    fn withdraw_cash_with_liquidation(&mut self, cash: &f64) -> StrategyEvent {
        if let BrokerCashEvent::WithdrawSuccess(withdrawn) =
            //No logging here because the implementation is fully logged due to the greater
            //complexity of this task vs standard withdraw
            self.brkr.withdraw_cash_with_liquidation(cash)
        {
            self.net_cash_flow = CashValue::from(*self.net_cash_flow - *withdrawn);
            return StrategyEvent::WithdrawSuccess(CashValue::from(*cash));
        }
        StrategyEvent::WithdrawFailure(CashValue::from(*cash))
    }
}

impl Audit<UistTrade> for StaticWeightStrategy {
    fn trades_between(&self, start: &i64, end: &i64) -> Vec<UistTrade> {
        self.brkr.trades_between(start, end)
    }
}

impl History for StaticWeightStrategy {
    fn get_history(&self) -> Vec<StrategySnapshot> {
        self.history.clone()
    }
}
