use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::clock::Clock;
use crate::input::penelope::{Penelope, PenelopeBuilder, PenelopeQuote};

// Unclear if the right approach is traits but this was the quickest way
pub trait UistSource {
    fn get_quote(&self, date: &i64, security: &str) -> Option<UistQuote>;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UistQuote {
    bid: f64,
    ask: f64,
    date: i64,
    symbol: String,
}

impl PenelopeQuote for UistQuote {
    fn get_ask(&self) -> f64 {
        self.ask
    }

    fn get_bid(&self) -> f64 {
        self.bid
    }

    fn get_date(&self) -> i64 {
        self.date
    }

    fn get_symbol(&self) -> String {
        self.symbol.clone()
    }

    fn create(bid: f64, ask: f64, date: i64, symbol: String) -> Self {
        Self {
            bid,
            ask,
            date,
            symbol,
        }
    }
}

pub type OrderId = u64;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TradeType {
    Buy,
    Sell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trade {
    pub symbol: String,
    pub value: f64,
    pub quantity: f64,
    pub date: i64,
    pub typ: TradeType,
}

impl Trade {
    pub fn new(
        symbol: impl Into<String>,
        value: f64,
        quantity: f64,
        date: i64,
        typ: TradeType,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            value,
            quantity,
            date,
            typ,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Order {
    pub order_id: Option<OrderId>,
    pub order_type: OrderType,
    pub symbol: String,
    pub shares: f64,
    pub price: Option<f64>,
}

impl Order {
    pub fn get_shares(&self) -> f64 {
        self.shares
    }

    pub fn get_symbol(&self) -> &str {
        &self.symbol
    }
    pub fn get_price(&self) -> &Option<f64> {
        &self.price
    }

    pub fn get_order_type(&self) -> &OrderType {
        &self.order_type
    }

    fn set_order_id(&mut self, order_id: u64) {
        self.order_id = Some(order_id);
    }

    fn market(order_type: OrderType, symbol: impl Into<String>, shares: f64) -> Self {
        Self {
            order_id: None,
            order_type,
            symbol: symbol.into(),
            shares,
            price: None,
        }
    }

    fn delayed(order_type: OrderType, symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Self {
            order_id: None,
            order_type,
            symbol: symbol.into(),
            shares,
            price: Some(price),
        }
    }

    pub fn market_buy(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketBuy, symbol, shares)
    }

    pub fn market_sell(symbol: impl Into<String>, shares: f64) -> Self {
        Order::market(OrderType::MarketSell, symbol, shares)
    }

    pub fn stop_buy(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::StopBuy, symbol, shares, price)
    }

    pub fn stop_sell(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::StopSell, symbol, shares, price)
    }

    pub fn limit_buy(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::LimitBuy, symbol, shares, price)
    }

    pub fn limit_sell(symbol: impl Into<String>, shares: f64, price: f64) -> Self {
        Order::delayed(OrderType::LimitSell, symbol, shares, price)
    }
}

impl Eq for Order {}

impl PartialEq for Order {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && self.order_type == other.order_type
            && self.shares == other.shares
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitMessage {
    pub start: i64,
    pub frequency: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InfoMessage {
    pub version: String,
    pub dataset: String,
}

impl InfoMessage {
    fn v1(dataset: String) -> InfoMessage {
        InfoMessage {
            version: "1.0".to_string(),
            dataset,
        }
    }
}

#[derive(Debug)]
pub struct UistV1 {
    dataset: String,
    clock: Clock,
    price_source: Penelope<UistQuote>,
    orderbook: OrderBook,
    trade_log: Vec<Trade>,
    //This is cleared on every tick
    order_buffer: Vec<Order>,
}

impl UistV1 {
    pub fn from_binance() -> Self {
        let (penelope, clock) = Penelope::from_binance();
        Self::new(clock, penelope, "BINANCE")
    }

    pub fn new(clock: Clock, price_source: Penelope<UistQuote>, dataset: &str) -> Self {
        Self {
            dataset: dataset.into(),
            clock,
            price_source,
            orderbook: OrderBook::default(),
            trade_log: Vec::new(),
            order_buffer: Vec::new(),
        }
    }

    fn sort_order_buffer(&mut self) {
        self.order_buffer.sort_by(|a, _b| match a.get_order_type() {
            OrderType::LimitSell | OrderType::StopSell | OrderType::MarketSell => {
                std::cmp::Ordering::Less
            }
            _ => std::cmp::Ordering::Greater,
        })
    }

    pub fn info(&self) -> InfoMessage {
        InfoMessage::v1(self.dataset.clone())
    }

    pub fn init(&self) -> InitMessage {
        InitMessage {
            start: *self.clock.now(),
            frequency: self.clock.frequency().clone().into(),
        }
    }

    pub fn fetch_quotes(&self) -> Vec<UistQuote> {
        if let Some(quotes) = self.price_source.get_quotes(&self.clock.now()) {
            return quotes;
        }
        vec![]
    }

    pub fn insert_order(&mut self, order: Order) {
        // Orders are only inserted into the book when tick is called, this is to ensure proper
        // ordering of trades
        // This impacts order_id where an order X can come in before order X+1 but the latter can
        // have an order_id that is less than the former.
        self.order_buffer.push(order);
    }

    pub fn delete_order(&mut self, order_id: OrderId) {
        self.orderbook.delete_order(order_id);
    }

    pub fn tick(&mut self) -> (bool, Vec<Trade>, Vec<Order>) {
        //To eliminate lookahead bias, we only start executing orders on the next
        //tick.
        self.clock.tick();

        self.sort_order_buffer();
        for order in self.order_buffer.iter_mut() {
            self.orderbook.insert_order(order);
        }

        let now = self.clock.now();
        let executed_trades = self.orderbook.execute_orders(*now, &self.price_source);
        for executed_trade in &executed_trades {
            self.trade_log.push(executed_trade.clone());
        }
        let inserted_orders = std::mem::take(&mut self.order_buffer);
        (self.clock.has_next(), executed_trades, inserted_orders)
    }
}

/// Generates random [Uist](UistV1) for use in tests that don't depend on prices.
pub fn random_uist_generator(length: i64) -> (UistV1, Clock) {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut source_builder = PenelopeBuilder::new();

    for date in 100..length + 100 {
        source_builder.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        source_builder.add_quote(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
    }

    let (penelope, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);
    (UistV1::new(clock.clone(), penelope, "RANDOM"), clock)
}

#[derive(Debug)]
pub struct OrderBook {
    inner: VecDeque<Order>,
    last_inserted: u64,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            inner: std::collections::VecDeque::new(),
            last_inserted: 0,
        }
    }

    pub fn delete_order(&mut self, delete_order_id: u64) {
        let mut delete_position: Option<usize> = None;
        for (position, order) in self.inner.iter().enumerate() {
            if let Some(order_id) = order.order_id {
                if order_id == delete_order_id {
                    delete_position = Some(position);
                    break;
                }
            }
        }
        if let Some(position) = delete_position {
            self.inner.remove(position);
        }
    }

    pub fn insert_order(&mut self, order: &mut Order) {
        order.set_order_id(self.last_inserted);
        self.inner.push_back(order.clone());
        self.last_inserted += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn execute_buy(quote: UistQuote, order: &Order, date: i64) -> Trade {
        let trade_price = quote.get_ask();
        let value = trade_price * order.get_shares();
        Trade {
            symbol: order.get_symbol().to_string(),
            value,
            quantity: order.get_shares(),
            date,
            typ: TradeType::Buy,
        }
    }

    fn execute_sell(quote: UistQuote, order: &Order, date: i64) -> Trade {
        let trade_price = quote.get_bid();
        let value = trade_price * order.get_shares();
        Trade {
            symbol: order.get_symbol().to_string(),
            value,
            quantity: order.get_shares(),
            date,
            typ: TradeType::Sell,
        }
    }

    pub fn execute_orders(&mut self, date: i64, source: &impl UistSource) -> Vec<Trade> {
        let mut completed_orderids = Vec::new();
        let mut trade_results = Vec::new();
        if self.is_empty() {
            return trade_results;
        }

        for order in self.inner.iter() {
            let security_id = &order.symbol;
            if let Some(quote) = source.get_quote(&date, security_id) {
                let result = match order.order_type {
                    OrderType::MarketBuy => Some(Self::execute_buy(quote, order, date)),
                    OrderType::MarketSell => Some(Self::execute_sell(quote, order, date)),
                    OrderType::LimitBuy => {
                        //Unwrap is safe because LimitBuy will always have a price
                        let order_price = order.price;
                        if order_price >= Some(quote.get_ask()) {
                            Some(Self::execute_buy(quote, order, date))
                        } else {
                            None
                        }
                    }
                    OrderType::LimitSell => {
                        //Unwrap is safe because LimitSell will always have a price
                        let order_price = order.price;
                        if order_price <= Some(quote.get_bid()) {
                            Some(Self::execute_sell(quote, order, date))
                        } else {
                            None
                        }
                    }
                    OrderType::StopBuy => {
                        //Unwrap is safe because StopBuy will always have a price
                        let order_price = order.price;
                        if order_price <= Some(quote.get_ask()) {
                            Some(Self::execute_buy(quote, order, date))
                        } else {
                            None
                        }
                    }
                    OrderType::StopSell => {
                        //Unwrap is safe because StopSell will always have a price
                        let order_price = order.price;
                        if order_price >= Some(quote.get_bid()) {
                            Some(Self::execute_sell(quote, order, date))
                        } else {
                            None
                        }
                    }
                };
                if let Some(trade) = &result {
                    completed_orderids.push(order.order_id.unwrap());
                    trade_results.push(trade.clone());
                }
            }
        }
        for order_id in completed_orderids {
            self.delete_order(order_id);
        }
        trade_results
    }
}

#[cfg(test)]
mod tests {
    use super::{OrderBook, UistQuote, UistV1};
    use crate::input::penelope::PenelopeBuilder;

    use super::{Order, OrderType, TradeType};
    use crate::clock::{Clock, Frequency};
    use crate::input::penelope::Penelope;

    fn setup_orderbook() -> (Clock, Penelope<UistQuote>) {
        let mut price_source_builder = PenelopeBuilder::new();
        price_source_builder.add_quote(101.0, 102.00, 100, "ABC".to_string());
        price_source_builder.add_quote(102.0, 103.00, 101, "ABC".to_string());
        price_source_builder.add_quote(105.0, 106.00, 102, "ABC".to_string());

        let (penelope, clock) = price_source_builder.build_with_frequency(Frequency::Second);
        (clock, penelope)
    }

    #[test]
    fn test_that_multiple_orders_will_execute() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::MarketBuy,
            symbol: "ABC".to_string(),
            shares: 25.0,
            price: None,
        };
        orderbook.insert_order(&mut order);
        let mut order1 = Order {
            order_id: None,
            order_type: OrderType::MarketBuy,
            symbol: "ABC".to_string(),
            shares: 25.0,
            price: None,
        };
        orderbook.insert_order(&mut order1);
        let mut order2 = Order {
            order_id: None,
            order_type: OrderType::MarketBuy,
            symbol: "ABC".to_string(),
            shares: 25.0,
            price: None,
        };
        orderbook.insert_order(&mut order2);
        let mut order3 = Order {
            order_id: None,
            order_type: OrderType::MarketBuy,
            symbol: "ABC".to_string(),
            shares: 25.0,
            price: None,
        };
        orderbook.insert_order(&mut order3);

        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::MarketBuy,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: None,
        };

        orderbook.insert_order(&mut order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Trade executes at 100 so trade price should be 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(trade.date, 100);
    }

    #[test]
    fn test_that_sell_market_executes() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::MarketSell,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: None,
        };

        orderbook.insert_order(&mut order);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Trade executes at 100 so trade price should be 101
        assert_eq!(trade.value / trade.quantity, 101.00);
        assert_eq!(trade.date, 100);
    }

    #[test]
    fn test_that_buy_limit_triggers_correctly() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::LimitBuy,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(95.0),
        };
        let mut order1 = Order {
            order_id: None,
            order_type: OrderType::LimitBuy,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(105.0),
        };

        orderbook.insert_order(&mut order);
        orderbook.insert_order(&mut order1);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Limit order has price of 105 but should execute at the ask, which is 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(trade.date, 100);
    }

    #[test]
    fn test_that_sell_limit_triggers_correctly() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::LimitSell,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(95.0),
        };
        let mut order1 = Order {
            order_id: None,
            order_type: OrderType::LimitSell,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(105.0),
        };

        orderbook.insert_order(&mut order);
        orderbook.insert_order(&mut order1);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Limit order has price of 95 but should execute at the ask, which is 101
        assert_eq!(trade.value / trade.quantity, 101.00);
        assert_eq!(trade.date, 100);
    }

    #[test]
    fn test_that_buy_stop_triggers_correctly() {
        //We are short from 90, and we put a StopBuy of 95 & 105 to take
        //off the position. If we are quoted 101/102 then 95 order
        //should be executed.

        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::StopBuy,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(95.0),
        };
        let mut order1 = Order {
            order_id: None,
            order_type: OrderType::StopBuy,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(105.0),
        };

        orderbook.insert_order(&mut order);
        orderbook.insert_order(&mut order1);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Stop order has price of 103 but should execute at the ask, which is 102
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(trade.date, 100);
    }

    #[test]
    fn test_that_sell_stop_triggers_correctly() {
        //Long from 110, we place orders to exit at 100 and 105.
        //If we are quoted 101/102 then our 105 StopSell is executed.

        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::StopSell,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(99.0),
        };
        let mut order1 = Order {
            order_id: None,
            order_type: OrderType::StopSell,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: Some(105.0),
        };

        orderbook.insert_order(&mut order);
        orderbook.insert_order(&mut order1);
        let mut executed = orderbook.execute_orders(100.into(), &source);
        //Only one order should execute on this tick
        assert_eq!(executed.len(), 1);

        let trade = executed.pop().unwrap();
        //Stop order has price of 105 but should execute at the bid, which is 101
        assert_eq!(trade.value / trade.quantity, 101.00);
        assert_eq!(trade.date, 100);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently_orderbook() {
        let (_clock, source) = setup_orderbook();
        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::MarketBuy,
            symbol: "XYZ".to_string(),
            shares: 100.0,
            price: None,
        };

        orderbook.insert_order(&mut order);
        let executed = orderbook.execute_orders(100.into(), &source);
        assert_eq!(executed.len(), 0);
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later_orderbook() {
        let mut price_source_builder = PenelopeBuilder::new();
        price_source_builder.add_quote(101.00, 102.00, 100, "ABC".to_string());
        price_source_builder.add_quote(105.00, 106.00, 102, "ABC".to_string());

        let (price_source, mut clock) =
            price_source_builder.build_with_frequency(Frequency::Second);

        clock.tick();

        let mut orderbook = OrderBook::new();
        let mut order = Order {
            order_id: None,
            order_type: OrderType::MarketBuy,
            symbol: "ABC".to_string(),
            shares: 100.0,
            price: None,
        };
        orderbook.insert_order(&mut order);
        let orders = orderbook.execute_orders(101.into(), &price_source);
        //Trades cannot execute without prices
        assert_eq!(orders.len(), 0);
        assert!(!orderbook.is_empty());

        clock.tick();
        //Order executes now with prices
        let mut orders = orderbook.execute_orders(102.into(), &price_source);
        assert_eq!(orders.len(), 1);

        let trade = orders.pop().unwrap();
        assert_eq!(trade.value / trade.quantity, 106.00);
        assert_eq!(trade.date, 102);
    }

    fn setup() -> UistV1 {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(101.00, 102.00, 100, "ABC".to_owned());
        source_builder.add_quote(102.00, 103.00, 101, "ABC".to_owned());
        source_builder.add_quote(105.00, 106.00, 102, "ABC".to_owned());

        let (source, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);

        let exchange = UistV1::new(clock, source, "FAKE");
        exchange
    }

    #[test]
    fn test_that_buy_market_executes_incrementing_trade_log() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick();

        //TODO: no abstraction!
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_same_tick() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));

        exchange.tick();
        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_multiple_orders_are_executed_on_consecutive_tick() {
        let mut exchange = setup();
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.tick();

        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.insert_order(Order::market_buy("ABC", 25.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 4);
    }

    #[test]
    fn test_that_buy_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 103.00);
        assert_eq!(trade.date, 101);
    }

    #[test]
    fn test_that_sell_market_executes_on_next_tick() {
        //Verifies that trades do not execute instaneously removing lookahead bias
        let mut exchange = setup();

        exchange.insert_order(Order::market_sell("ABC", 100.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 1);
        let trade = exchange.trade_log.remove(0);
        //Trade executes at 101 so trade price should be 103
        assert_eq!(trade.value / trade.quantity, 102.00);
        assert_eq!(trade.date, 101);
    }

    #[test]
    fn test_that_order_for_nonexistent_stock_fails_silently() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy("XYZ", 100.0));
        exchange.tick();

        assert_eq!(exchange.trade_log.len(), 0);
    }

    #[test]
    fn test_that_order_buffer_clears() {
        //Sounds redundant but accidentally removing the clear could cause unusual errors elsewhere
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick();

        assert!(exchange.order_buffer.is_empty());
    }

    #[test]
    fn test_that_order_with_missing_price_executes_later() {
        let mut source_builder = PenelopeBuilder::new();
        source_builder.add_quote(101.00, 102.00, 100, "ABC".to_owned());
        source_builder.add_quote(105.00, 106.00, 102, "ABC".to_owned());

        let (source, clock) = source_builder.build_with_frequency(crate::clock::Frequency::Second);

        let mut exchange = UistV1::new(clock, source, "FAKE");

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.tick();
        //Orderbook should have one order and trade log has no executed trades
        assert_eq!(exchange.trade_log.len(), 0);

        exchange.tick();
        //Order should execute now
        assert_eq!(exchange.trade_log.len(), 1);
    }

    #[test]
    fn test_that_sells_are_executed_before_buy() {
        let mut exchange = setup();

        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.insert_order(Order::market_buy("ABC", 100.0));
        exchange.insert_order(Order::market_sell("ABC", 100.0));
        let res = exchange.tick();

        assert_eq!(res.1.len(), 3);
        assert_eq!(res.1.get(0).unwrap().typ, TradeType::Sell)
    }
}
