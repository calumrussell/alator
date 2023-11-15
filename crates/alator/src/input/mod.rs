//! Data sources

use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use std::collections::HashMap;
use std::sync::Arc;

use crate::broker::{Dividend, Quote};
use crate::types::Price;
use alator_clock::{Clock, DateTime};

#[cfg(feature = "python")]
use crate::broker::{PyDividend, PyQuote};
#[cfg(feature = "python")]
use pyo3::pycell::PyCell;
#[cfg(feature = "python")]
use pyo3::types::{PyDict, PyList};

/// Inner type for [PriceSource].
pub trait Quotable: Clone + std::marker::Send + std::marker::Sync {
    fn get_bid(&self) -> &Price;
    fn get_ask(&self) -> &Price;
    fn get_date(&self) -> &DateTime;
    fn get_symbol(&self) -> &String;
}

/// Inner type for dividends for [CorporateEventsSource].
pub trait Dividendable: Clone + std::marker::Send + std::marker::Sync {
    fn get_symbol(&self) -> &String;
    fn get_date(&self) -> &DateTime;
    fn get_value(&self) -> &Price;
}

/// Represents structure that generates price quotes.
///
/// Related to [Quotable]. Other components are tightly bound to the types that implement these
/// traits.
///
/// Whilst this can be cloned, users should be aware that cloning multiple times will likely be
/// one of the most expensive operations in a backtest so care should be taken to minimize these
/// operations.
pub trait PriceSource<Q>: Clone
where
    Q: Quotable,
{
    fn get_quote(&self, symbol: &str) -> Option<Arc<Q>>;
    fn get_quotes(&self) -> Option<Vec<Arc<Q>>>;
}

/// Represents structure that generates dividend information.
///
/// There can be multiple types of corporate events but we currently only support dividends.
pub trait CorporateEventsSource<D>: Clone
where
    D: Dividendable,
{
    fn get_dividends(&self) -> Option<Vec<Arc<D>>>;
}

type DefaultPriceSourceImpl<Q> = (HashMap<DateTime, Vec<Arc<Q>>>, Clock);

/// Default implementation of [PriceSource] using [Quote] as inner type.
///
/// This implementation is thread-safe but users should consider the conditions under which
/// multiple threads should be accesssing prices. In library implementations, this is tightly
/// controlled for performance/simplicity reasons with the exchange being the only source.
#[derive(Debug)]
pub struct DefaultPriceSource {
    //It isn't strictly necessary that this access is thread-safe as exchange is the only price
    //source but this protects new implementations.
    inner: Arc<DefaultPriceSourceImpl<Quote>>,
}

impl PriceSource<Quote> for DefaultPriceSource {
    fn get_quote(&self, symbol: &str) -> Option<Arc<Quote>> {
        let curr_date = self.inner.1.now();
        if let Some(quotes) = self.inner.0.get(&curr_date) {
            for quote in quotes {
                if quote.get_symbol().eq(symbol) {
                    return Some(quote.clone());
                }
            }
        }
        None
    }

    fn get_quotes(&self) -> Option<Vec<Arc<Quote>>> {
        let curr_date = self.inner.1.now();
        if let Some(quotes) = self.inner.0.get(&curr_date) {
            return Some(quotes.clone());
        }
        None
    }
}

impl Clone for DefaultPriceSource {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DefaultPriceSource {
    pub fn add_quotes(
        &mut self,
        bid: impl Into<Price>,
        ask: impl Into<Price>,
        date: impl Into<DateTime>,
        symbol: impl Into<String>,
    ) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        let datetime: DateTime = date.into();

        let quote = Quote::new(bid, ask, datetime, symbol);
        if let Some(quotes) = inner.0.get_mut(&datetime) {
            quotes.push(Arc::new(quote))
        } else {
            inner.0.insert(datetime, vec![Arc::new(quote)]);
        }
    }

    pub fn from_hashmap(quotes: HashMap<DateTime, Vec<Arc<Quote>>>, clock: Clock) -> Self {
        Self {
            inner: Arc::new((quotes, clock)),
        }
    }

    pub fn new(clock: Clock) -> Self {
        let quotes = HashMap::with_capacity(clock.len());
        Self {
            inner: Arc::new((quotes, clock)),
        }
    }
}

#[cfg(feature = "python")]
#[derive(Clone, Debug)]
pub struct PyPriceSource<'a> {
    pub quotes: &'a PyDict,
    pub tickers: &'a PyDict,
    pub clock: Clock,
}

#[cfg(feature = "python")]
impl<'a> PriceSource<PyQuote> for PyPriceSource<'a> {
    fn get_quote(&self, symbol: &str) -> Option<Arc<PyQuote>> {
        if let Ok(Some(ticker_pos_any)) = self.tickers.get_item(symbol) {
            let curr_date = self.clock.now();
            if let Ok(Some(quotes)) = self.quotes.get_item(i64::from(curr_date)) {
                if let Ok(quotes_list) = quotes.downcast::<PyList>() {
                    if let Ok(ticker_pos) = ticker_pos_any.extract::<usize>() {
                        let quote_any = &quotes_list[ticker_pos];
                        if let Ok(quote) = quote_any.downcast::<PyCell<PyQuote>>() {
                            let to_inner = quote.get();
                            return Some(Arc::new(to_inner.clone()));
                        }
                    }
                }
            }
        }
        None
    }

    //TODO: need to implement, can't do this without Python-native types
    fn get_quotes(&self) -> Option<Vec<Arc<PyQuote>>> {
        None
    }
}

#[cfg(feature = "python")]
#[derive(Clone, Debug)]
pub struct PyCorporateEventsSource<'a> {
    pub dividends: &'a PyDict,
    pub clock: Clock,
}

#[cfg(feature = "python")]
impl<'a> CorporateEventsSource<PyDividend> for PyCorporateEventsSource<'a> {
    fn get_dividends(&self) -> Option<Vec<Arc<PyDividend>>> {
        None
    }
}

type CorporateEventsSourceImpl<D> = (HashMap<DateTime, Vec<Arc<D>>>, Clock);

/// Default implementation of [CorporateEventsSource] with [Dividend] as inner type.
#[derive(Debug)]
pub struct DefaultCorporateEventsSource {
    inner: std::sync::Arc<CorporateEventsSourceImpl<Dividend>>,
}

impl CorporateEventsSource<Dividend> for DefaultCorporateEventsSource {
    fn get_dividends(&self) -> Option<Vec<Arc<Dividend>>> {
        let curr_date = self.inner.1.now();
        if let Some(dividends) = self.inner.0.get(&curr_date) {
            return Some(dividends.clone());
        }
        None
    }
}

impl Clone for DefaultCorporateEventsSource {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DefaultCorporateEventsSource {
    pub fn add_dividends(
        &mut self,
        value: impl Into<Price>,
        symbol: impl Into<String>,
        date: impl Into<DateTime>,
    ) {
        let inner = Arc::get_mut(&mut self.inner).unwrap();
        let datetime: DateTime = date.into();
        let dividend = Dividend::new(value, symbol, datetime);

        if let Some(dividends) = inner.0.get_mut(&datetime) {
            dividends.push(Arc::new(dividend));
        } else {
            inner.0.insert(datetime, vec![Arc::new(dividend)]);
        }
    }

    pub fn new(clock: Clock) -> Self {
        let quotes = HashMap::with_capacity(clock.len());
        Self {
            inner: Arc::new((quotes, clock)),
        }
    }
}

/// Generates random [DefaultPriceSource] for use in tests that don't depend on prices.
pub fn fake_price_source_generator(clock: Clock) -> DefaultPriceSource {
    let price_dist = Uniform::new(90.0, 100.0);
    let mut rng = thread_rng();

    let mut price_source = DefaultPriceSource::new(clock.clone());
    for date in clock.peek() {
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "ABC",
        );
        price_source.add_quotes(
            price_dist.sample(&mut rng),
            price_dist.sample(&mut rng),
            date,
            "BCD",
        );
    }
    price_source
}