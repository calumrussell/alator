# What is Alator?

Rust library with components for investment portfolio backtesting.

This library is used as a back-end for a Python financial simulation [app](https://pytho.uk). A feature of this application is Monte-Carlo simulations of investor lifetimes, and to run hundreds of these complex, event-driven simulations within a few seconds requires a fast backtesting engine.

# How does Alator work?

The primary development goal is providing a simple, flexible backtesting library that doesn't have huge performance sacrifices.

Most of the logic lies within `Strategy` and `Broker`: a strategy tells the broker what trades to execute, and a broker executes them. Dependencies on outside data are not shared, as there are many cases when components require different data. The only shared dependence is `Clock` which tells other components the current point in a backtest.

A conscious choice was made not to completely split `Strategy` and `Broker`, as is common in event-driven architectures having each component communicate with the other through messages. This approach would provide superior horizontal scalability in a live HFT environment but at the cost of some duplication of code/responsibility and complexity. Whilst this library could be used in production, and some components (for example, `Clock`) are designed with this in mind, it is not a primary focus, so there is no need to think about scalability beyond the confines of a single backtest run. And, hopefully, the code is easier to understand too.

An example backtest (with data creation excluded):

```
    let initial_cash: CashValue = 100_000.0.into();
    let length_in_days: i64 = 200;
    let start_date: i64 = 1609750800; //Date - 4/1/21 9:00:0000
    let clock = ClockBuilder::from_length(&start_date.into(), length_in_days).daily();

    let data = build_data(Rc::clone(&clock));

    let mut weights: PortfolioAllocation<PortfolioWeight> = PortfolioAllocation::new();
    weights.insert(&String::from("ABC"), &0.5.into());
    weights.insert(&String::from("BCD"), &0.5.into());

    let simbrkr = SimulatedBrokerBuilder::new()
        .with_data(data)
        .with_trade_costs(vec![BrokerCost::Flat(1.0.into())])
        .build();

    let strat = StaticWeightStrategyBuilder::new()
        .with_brkr(simbrkr)
        .with_weights(weights)
        .with_clock(Rc::clone(&clock))
        .daily();

    let mut sim = SimContextBuilder::new()
        .with_clock(Rc::clone(&clock))
        .with_strategy(strat)
        .init(&initial_cash);

    sim.run();
```
Alator comes with a `StaticWeightStrategy` and clients will typically need to implement the `Strategy` trait to build a new strategy. The `Broker` component should be reusable for most cases but may lack the features for some use-cases, the three biggest being: leverage, multiple-currency support, and shorting. Because of the current uses of the library, the main priority in the near future is to add support for multiple-currencies.

# How do you get data into Alator for backtesting?

Alator is flexible about the underlying representation of data: at the bottom is just an implementation of a broker that operates on `Quote` structures. So all you need to provide the broker is a structure that implements `DataSource` and which can be queried to find `Quote` events (or any kind of broker-related event: for example, dividends). You can use ticks, you can use candlesticks, there is no dependency on underlying structure within the default broker implementation (but it is often the case that the strategy you implement will have a dependency on the data structure used i.e. a moving average system contains some assumptions about data frequency).

In the tests folder, we have provided an implementation of a simple moving average crossover strategy that pulls data directly from Binance. To see the system running: fail the test with an assert and pass `RUST_LOG=info` to the command.

# Missing features that you may expect

* Leverage
* Multi-currency/Multi-exchange support
* Shorting 
* Performance benchmarks
* Concurrency

The main priority in the near future, given the existing uses of the library, is support for multiple currencies and performance.

# Change Log

v0.1.6 - When broker attempts to query the value of a security without price and fails to find a quote for the current time, it will use the cached last seen bid instead of returning no value. This has changed the mutability guarantees of certain operations - for example, get_position_value - that sound like they should be immutable. Added tests to verify working functionality.

v0.1.5 - Backtest won't panic when the client generates target weights containing a security without a quote for the current date. Assumes that the client knows that the security will be tradable at some point, just not today (for example, weekends). If the client has created a target weight for a non-existent security, no error will be thrown. This reversion is required to account for non-trading days/bank holidays/etc. without requiring an ex-ante calculation of these dates. Added test cases for this scenario, added test to panic when diffing against a zero-value broker, added test to verify the behaviour of ClockBuilder when creating a fixed date Clock.

v0.1.4 - Added Clone to `PerfStruct`.

v0.1.3 - Clock creation methods are more descriptive, and define more clearly what kind of frequency and time scale the clock should run at.

v0.1.2 - Transfer traits in strategy now return events to caller that communicate the result of the operation.

v0.1.1 - Added test case showing how to create data source. Changed behaviour of `PositionInfo`.`get_portfolio_qty` so that ambiguous zero values aren't shown to client. 
