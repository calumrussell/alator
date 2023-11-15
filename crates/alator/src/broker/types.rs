#[cfg(feature = "python")]
use pyo3::{pyclass, pymethods};

use crate::{
    input::Dividendable,
    types::{CashValue, PortfolioQty, Price},
};
use alator_clock::DateTime;
use alator_exchange::{ ExchangeOrder, ExchangeOrderMessage };

#[allow(unused)]
use crate::types::PortfolioAllocation;

//Contains data structures and traits that refer solely to the data held and operations required
//for broker implementations.

///Represents a single dividend payment in per-share terms.
///
///Equality checked against ticker and date. Ordering against date only.
///
///let d = Dividend::new(
///  0.1,
///  "ABC"
///  100,
///);
#[derive(Clone, Debug)]
pub struct Dividend {
    //Dividend value is expressed in terms of per share values
    pub value: Price,
    pub symbol: String,
    pub date: DateTime,
}

impl Dividend {
    pub fn new(
        value: impl Into<Price>,
        symbol: impl Into<String>,
        date: impl Into<DateTime>,
    ) -> Self {
        Self {
            value: value.into(),
            symbol: symbol.into(),
            date: date.into(),
        }
    }
}

impl Ord for Dividend {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for Dividend {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Dividend {}

impl PartialEq for Dividend {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

impl Dividendable for Dividend {
    fn get_date(&self) -> &DateTime {
        &self.date
    }

    fn get_symbol(&self) -> &String {
        &self.symbol
    }

    fn get_value(&self) -> &Price {
        &self.value
    }
}

///Represents a single dividend payment in cash terms. Type is used internally within broker and
///is used only to credit the cash balance. Shouldn't be used outside a broker impl.
///
///Equality checked against ticker and date. Ordering against date only.
///
///let dp = DividendPayment::new(
///  0.1,
///  "ABC",
///  100,
///);
#[derive(Clone, Debug)]
pub struct DividendPayment {
    pub value: CashValue,
    pub symbol: String,
    pub date: DateTime,
}

impl DividendPayment {
    pub fn new(
        value: impl Into<CashValue>,
        symbol: impl Into<String>,
        date: impl Into<DateTime>,
    ) -> Self {
        Self {
            value: value.into(),
            symbol: symbol.into(),
            date: date.into(),
        }
    }
}

impl Ord for DividendPayment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for DividendPayment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for DividendPayment {}

impl PartialEq for DividendPayment {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

impl From<alator_exchange::TradeType> for TradeType {
    fn from(value: alator_exchange::TradeType) -> Self {
        match value {
            alator_exchange::TradeType::Buy => TradeType::Buy,
            alator_exchange::TradeType::Sell => TradeType::Sell,
        }
    }
}

///Represents a completed trade to be stored in the internal broker impl ledger or used by the
///client. This type is a pure internal representation, and clients do not pass trades to the
///broker to execute but pass an [Order] instaed.
///
///Equality checked against ticker, date, and quantity. Ordering against date only.
///
///let t = Trade::new(
///  "ABC",
///  100.0,
///  1000,
///  100,
///  TradeType::Buy,
///);
#[derive(Clone, Debug)]
pub struct Trade {
    //TODO: more indirection is needed for this type, possibly implemented as trait
    pub symbol: String,
    pub value: CashValue,
    pub quantity: PortfolioQty,
    pub date: DateTime,
    pub typ: TradeType,
}

impl Trade {
    pub fn new(
        symbol: impl Into<String>,
        value: impl Into<CashValue>,
        quantity: impl Into<PortfolioQty>,
        date: impl Into<DateTime>,
        typ: TradeType,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            value: value.into(),
            quantity: quantity.into(),
            date: date.into(),
            typ,
        }
    }
}

impl Ord for Trade {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.date.cmp(&other.date)
    }
}

impl PartialOrd for Trade {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Trade {}

impl PartialEq for Trade {
    fn eq(&self, other: &Self) -> bool {
        self.date == other.date && self.symbol == other.symbol
    }
}

impl From<alator_exchange::ExchangeTrade> for Trade {
    fn from(value: alator_exchange::ExchangeTrade) -> Self {
        Self {
            symbol: value.symbol,
            date: value.date,
            quantity: value.quantity.into(),
            typ: value.typ.into(),
            value: value.value.into(),
        }
    }
}

///Events generated by broker in the course of executing transactions.
///
///Brokers have two sources of state: holdings of stock and cash. Events represent modifications of
///that state over time. The vast majority, but not all, of these events could be returned to client
///applications.
#[derive(Clone, Debug)]
pub enum BrokerEvent {
    OrderSentToExchange(Order),
    OrderInvalid(Order),
    OrderCreated(Order),
    OrderFailure(Order),
}

#[derive(Clone, Debug)]
pub enum BrokerCashEvent {
    //Removed from [BrokerEvent] because there are situations when we want to handle these events
    //specifically and seperately
    WithdrawSuccess(CashValue),
    WithdrawFailure(CashValue),
    DepositSuccess(CashValue),
    OperationFailure(CashValue),
}

///Events generated by broker in the course of executing internal transactions.
///
///These events will typically only be used internally to return information to clients. In
///practice, these are currently used to record taxable events.
#[derive(Clone, Debug)]
pub enum BrokerRecordedEvent {
    TradeCompleted(Trade),
    DividendPaid(DividendPayment),
}

impl From<Trade> for BrokerRecordedEvent {
    fn from(trade: Trade) -> Self {
        BrokerRecordedEvent::TradeCompleted(trade)
    }
}

impl From<DividendPayment> for BrokerRecordedEvent {
    fn from(divi: DividendPayment) -> Self {
        BrokerRecordedEvent::DividendPaid(divi)
    }
}

///Represents the order types that a broker implementation should support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderType {
    MarketSell,
    MarketBuy,
    LimitSell,
    LimitBuy,
    StopSell,
    StopBuy,
}

impl From<alator_exchange::OrderType> for OrderType {
    fn from(value: alator_exchange::OrderType) -> Self {
        match value {
            alator_exchange::OrderType::LimitBuy => OrderType::LimitBuy,
            alator_exchange::OrderType::LimitSell => OrderType::LimitSell,
            alator_exchange::OrderType::MarketBuy => OrderType::MarketBuy,
            alator_exchange::OrderType::MarketSell => OrderType::MarketSell,
            alator_exchange::OrderType::StopBuy => OrderType::StopBuy,
            alator_exchange::OrderType::StopSell => OrderType::StopSell,
        }
    }
}

impl From<OrderType> for alator_exchange::OrderType {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::LimitBuy => alator_exchange::OrderType::LimitBuy,
            OrderType::LimitSell => alator_exchange::OrderType::LimitSell,
            OrderType::MarketBuy => alator_exchange::OrderType::MarketBuy,
            OrderType::MarketSell => alator_exchange::OrderType::MarketSell,
            OrderType::StopBuy => alator_exchange::OrderType::StopBuy,
            OrderType::StopSell => alator_exchange::OrderType::StopSell,
        }
    }
}

///Represents an order that is sent to a broker to execute. Trading strategies can send orders to
///brokers to execute. In practice, trading strategies typically target [PortfolioAllocation] but
///these allocations are just wrappers around [Order] that we diff against with the trading logic.
///
///Current execution model is to execute orders instaneously so there is no functional difference
///between a trade and a order: all orders eventually become trades. At some point, it is likely
///that the library moves away from this model so it makes sense to distinguish here between orders
///and trades.
///
///Equality checked against ticker, order_type, and quantity. No ordering.
///
///let o = Order::market(
///  OrderType::MarketBuy,
///  "ABC",
///  100.0,
///);
///
///let o1 = Order::delayed(
///  OrderType::StopSell,
///  "ABC",
///  100.0,
///  10.0,
///);
#[derive(Clone, Debug)]
pub struct Order {
    order_type: OrderType,
    symbol: String,
    shares: PortfolioQty,
    price: Option<Price>,
}

impl Order {
    //TODO: should this be a trait?
    pub fn get_symbol(&self) -> &String {
        &self.symbol
    }

    pub fn get_shares(&self) -> &PortfolioQty {
        &self.shares
    }

    pub fn get_price(&self) -> &Option<Price> {
        &self.price
    }

    pub fn get_order_type(&self) -> &OrderType {
        &self.order_type
    }

    pub fn market(
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: impl Into<PortfolioQty>,
    ) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            shares: shares.into(),
            price: None,
        }
    }

    pub fn delayed(
        order_type: OrderType,
        symbol: impl Into<String>,
        shares: impl Into<PortfolioQty>,
        price: impl Into<Price>,
    ) -> Self {
        Self {
            order_type,
            symbol: symbol.into(),
            shares: shares.into(),
            price: Some(price.into()),
        }
    }

    pub fn into_exchange(&self, subscriber_id: alator_exchange::DefaultSubscriberId) -> ExchangeOrder {
        let price: Option<f64> = self.get_price().as_ref().map(|price| (**price));

        ExchangeOrder {
            subscriber_id,
            price,
            shares: **self.get_shares(),
            symbol: self.get_symbol().to_string(),
            order_type: (*self.get_order_type()).into(),
        }
    }

    pub fn into_exchange_message(
        &self,
        subscriber_id: alator_exchange::DefaultSubscriberId,
    ) -> ExchangeOrderMessage {
        let price: Option<f64> = self.get_price().as_ref().map(|price| (**price));

        ExchangeOrderMessage::CreateOrder(ExchangeOrder {
            subscriber_id,
            price,
            shares: **self.get_shares(),
            symbol: self.get_symbol().to_string(),
            order_type: (*self.get_order_type()).into(),
        })
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

impl From<ExchangeOrder> for Order {
    fn from(value: ExchangeOrder) -> Self {
        let price: Option<Price> = value.get_price().as_ref().map(|price| (*price).into());
        Self {
            order_type: (*value.get_order_type()).into(),
            symbol: value.get_symbol().into(),
            shares: (*value.get_shares()).into(),
            price,
        }
    }
}

///Implementation of various cost models for brokers. Broker implementations would either define or
///cost model or would provide the user the option of intializing one; the broker impl would then
///call the variant's calculation methods as trades are executed.
#[derive(Clone, Debug)]
pub enum BrokerCost {
    PerShare(Price),
    PctOfValue(f64),
    Flat(CashValue),
}

impl BrokerCost {
    pub fn per_share(val: f64) -> Self {
        BrokerCost::PerShare(Price::from(val))
    }

    pub fn pct_of_value(val: f64) -> Self {
        BrokerCost::PctOfValue(val)
    }

    pub fn flat(val: f64) -> Self {
        BrokerCost::Flat(CashValue::from(val))
    }

    pub fn calc(&self, trade: &Trade) -> CashValue {
        match self {
            BrokerCost::PerShare(cost) => CashValue::from(*cost.clone() * *trade.quantity.clone()),
            BrokerCost::PctOfValue(pct) => CashValue::from(*trade.value * *pct),
            BrokerCost::Flat(val) => val.clone(),
        }
    }

    //Returns a valid trade given trading costs given a current budget
    //and price of security
    pub fn trade_impact(
        &self,
        gross_budget: &f64,
        gross_price: &f64,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut net_budget = *gross_budget;
        let mut net_price = *gross_price;
        match self {
            BrokerCost::PerShare(val) => {
                if is_buy {
                    net_price += *val.clone();
                } else {
                    net_price -= *val.clone();
                }
            }
            BrokerCost::PctOfValue(pct) => {
                net_budget *= 1.0 - pct;
            }
            BrokerCost::Flat(val) => net_budget -= *val.clone(),
        }
        (CashValue::from(net_budget), Price::from(net_price))
    }

    pub fn trade_impact_total(
        trade_costs: &[BrokerCost],
        gross_budget: &f64,
        gross_price: &f64,
        is_buy: bool,
    ) -> (CashValue, Price) {
        let mut res = (CashValue::from(*gross_budget), Price::from(*gross_price));
        for cost in trade_costs {
            res = cost.trade_impact(&res.0, &res.1, is_buy);
        }
        res
    }
}
