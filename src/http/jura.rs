use std::collections::HashMap;

use crate::{exchange::jura_v1::{Fill, JuraV1, Order, OrderId}, input::penelope::{Penelope, PenelopeQuoteByDate}};

type BacktestId = u64;

pub struct BacktestState {
    pub id: BacktestId,
    pub date: i64,
    pub exchange: JuraV1,
    pub dataset_name: String,
}

pub struct AppState {
    pub backtests: HashMap<BacktestId, BacktestState>,
    pub last: BacktestId,
    pub datasets: HashMap<String, Penelope>,
}

impl AppState {
    pub fn create(datasets: &mut HashMap<String, Penelope>) -> Self {
        Self {
            backtests: HashMap::new(),
            last: 0,
            datasets: std::mem::take(datasets),
        }
    }

    pub fn single(name: &str, data: Penelope) -> Self {
        let exchange = JuraV1::new();
        let backtest = BacktestState {
            id: 0,
            date: data.get_first_date().clone(),
            exchange,
            dataset_name: name.into(),
        };

        let mut datasets = HashMap::new();
        datasets.insert(name.into(), data);

        let mut backtests = HashMap::new();
        backtests.insert(0, backtest);

        Self {
            backtests,
            last: 1,
            datasets,
        }
    }

    pub fn tick(&mut self, backtest_id: BacktestId) -> Option<(Vec<Fill>, Vec<Order>, Vec<u64>)> {
        if let Some(backtest) = self.backtests.get_mut(&backtest_id) {
            if let Some(dataset) = self.datasets.get(&backtest.dataset_name) {
                if let Some(quotes) = dataset.get_quotes(&backtest.date) {
                    let next_date = dataset.get_next_date(&backtest.date);

                    let res = backtest.exchange.tick(&quotes);
                    backtest.date = next_date.clone();
                    return Some((res.0, res.1, res.2));
                }
            }
        }
        None
    }

    pub fn fetch_quotes(&self, backtest_id: BacktestId) -> Option<&PenelopeQuoteByDate> {
        if let Some(backtest) = self.backtests.get(&backtest_id) {
            if let Some(dataset) = self.datasets.get(&backtest.dataset_name) {
                return dataset.get_quotes(&backtest.date);
            }
        }
        None
    }

    pub fn init(&mut self, dataset_name: String) -> Option<BacktestId> {
        if let Some(dataset) = self.datasets.get(&dataset_name) {
            let new_id = self.last + 1;
            let exchange = JuraV1::new();
            let backtest = BacktestState {
                id: new_id,
                date: dataset.get_first_date().clone(),
                exchange,
                dataset_name: dataset_name.into(),
            };
            self.backtests.insert(new_id, backtest);
            return Some(new_id);
        }
        None
    }

    pub fn insert_order(&mut self, order: Order, backtest_id: BacktestId) -> Option<()> {
        if let Some(backtest) = self.backtests.get_mut(&backtest_id) {
            backtest.exchange.insert_order(order);
            return Some(());
        }
        None
    }

    pub fn delete_order(&mut self, asset: u64, order_id: OrderId, backtest_id: BacktestId) -> Option<()> {
        if let Some(backtest) = self.backtests.get_mut(&backtest_id) {
            backtest.exchange.delete_order(asset, order_id);
            return Some(());
        }
        None
    }

    pub fn new_backtest(&mut self, dataset_name: &str) -> Option<BacktestId> {
        let new_id = self.last + 1;

        // Check that dataset exists
        if let Some(dataset) = self.datasets.get(dataset_name) {
            let exchange = JuraV1::new();

            let backtest = BacktestState {
                id: new_id,
                date: dataset.get_first_date().clone(),
                exchange,
                dataset_name: dataset_name.into(),
            };

            self.backtests.insert(new_id, backtest);

            self.last = new_id;
            return Some(new_id);
        }
        None
    }
}

pub mod jurav1_client {

    use reqwest::Result;

    use super::jurav1_server::{
        DeleteOrderRequest, FetchQuotesResponse, InsertOrderRequest, TickResponse,
    };

    use crate::exchange::jura_v1::{InfoMessage, InitMessage, Order, OrderId};

    type BacktestId = u64;

    pub struct Client {
        pub path: String,
        pub client: reqwest::Client,
    }

    impl Client {
        pub async fn tick(&self, backtest_id: BacktestId) -> Result<TickResponse> {
            self.client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/tick").as_str())
                .send()
                .await?
                .json::<TickResponse>()
                .await
        }

        pub async fn delete_order(
            &self,
            asset: u64,
            order_id: OrderId,
            backtest_id: BacktestId,
        ) -> Result<()> {
            let req = DeleteOrderRequest { asset, order_id };
            self.client
                .post(self.path.clone() + format!("/backtest/{backtest_id}/delete_order").as_str())
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await
        }

        pub async fn insert_order(&self, order: Order, backtest_id: BacktestId) -> Result<()> {
            let req = InsertOrderRequest { order };
            self.client
                .post(self.path.clone() + format!("/backtest/{backtest_id}/insert_order").as_str())
                .json(&req)
                .send()
                .await?
                .json::<()>()
                .await
        }

        pub async fn fetch_quotes(&self, backtest_id: BacktestId) -> Result<FetchQuotesResponse> {
            self.client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/fetch_quotes").as_str())
                .send()
                .await?
                .json::<FetchQuotesResponse>()
                .await
        }

        pub async fn init(&self, dataset_name: String) -> Result<InitMessage> {
            self.client
                .get(self.path.clone() + format!("/init/{dataset_name}").as_str())
                .send()
                .await?
                .json::<InitMessage>()
                .await
        }

        pub async fn info(&self, backtest_id: BacktestId) -> Result<InfoMessage> {
            self.client
                .get(self.path.clone() + format!("/backtest/{backtest_id}/info").as_str())
                .send()
                .await?
                .json::<InfoMessage>()
                .await
        }

        pub fn new(path: String) -> Self {
            Self {
                path,
                client: reqwest::Client::new(),
            }
        }
    }
}

pub mod jurav1_server {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::sync::Mutex;

    use crate::exchange::jura_v1::{Fill, InitMessage, JuraQuote, JuraV1, Order, OrderId};
    use actix_web::{
        error, get, post,
        web::{self, Path},
        Result,
    };
    use derive_more::{Display, Error};

    use super::AppState;

    type BacktestId = u64;
    pub type JuraState = Mutex<AppState>;


    #[derive(Debug, Display, Error)]
    pub enum JuraV1Error {
        UnknownBacktest,
        UnknownDataset,
    }

    impl error::ResponseError for JuraV1Error {
        fn status_code(&self) -> actix_web::http::StatusCode {
            match self {
                JuraV1Error::UnknownBacktest => actix_web::http::StatusCode::BAD_REQUEST,
                JuraV1Error::UnknownDataset => actix_web::http::StatusCode::BAD_REQUEST,
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct TickResponse {
        pub has_next: bool,
        pub executed_trades: Vec<Fill>,
        pub inserted_orders: Vec<Order>,
    }

    #[get("/backtest/{backtest_id}/tick")]
    pub async fn tick(
        app: web::Data<JuraState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<TickResponse>, JuraV1Error> {
        let mut jura = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(state) = jura.exchanges.get_mut(&backtest_id) {
            let tick = state.exchange.tick();
            Ok(web::Json(TickResponse {
                inserted_orders: tick.2,
                executed_trades: tick.1,
                has_next: tick.0,
            }))
        } else {
            Err(JuraV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct DeleteOrderRequest {
        pub asset: u64,
        pub order_id: OrderId,
    }

    #[post("/backtest/{backtest_id}/delete_order")]
    pub async fn delete_order(
        app: web::Data<JuraState>,
        path: web::Path<(BacktestId,)>,
        delete_order: web::Json<DeleteOrderRequest>,
    ) -> Result<web::Json<()>, JuraV1Error> {
        let mut jura = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();
        if let Some(state) = jura.exchanges.get_mut(&backtest_id) {
            state
                .exchange
                .delete_order(delete_order.asset, delete_order.order_id);
            Ok(web::Json(()))
        } else {
            Err(JuraV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InsertOrderRequest {
        pub order: Order,
    }

    #[post("/backtest/{backtest_id}/insert_order")]
    pub async fn insert_order(
        app: web::Data<JuraState>,
        path: Path<(BacktestId,)>,
        insert_order: web::Json<InsertOrderRequest>,
    ) -> Result<web::Json<()>, JuraV1Error> {
        let mut jura = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(state) = jura.exchanges.get_mut(&backtest_id) {
            state.exchange.insert_order(insert_order.order.clone());
            Ok(web::Json(()))
        } else {
            Err(JuraV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct FetchQuotesResponse {
        pub quotes: Vec<JuraQuote>,
    }

    #[get("/backtest/{backtest_id}/fetch_quotes")]
    pub async fn fetch_quotes(
        app: web::Data<JuraState>,
        path: Path<(BacktestId,)>,
    ) -> Result<web::Json<FetchQuotesResponse>, JuraV1Error> {
        let mut jura = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(state) = jura.exchanges.get_mut(&backtest_id) {
            Ok(web::Json(FetchQuotesResponse {
                quotes: state.exchange.fetch_quotes(),
            }))
        } else {
            Err(JuraV1Error::UnknownBacktest)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InitResponse {
        pub backtest_id: BacktestId,
        pub start: i64,
        pub frequency: u8,
    }

    #[get("/init/{dataset_name}")]
    pub async fn init(
        app: web::Data<JuraState>,
        path: Path<(String,)>,
    ) -> Result<web::Json<InitResponse>, JuraV1Error> {
        let mut jura = app.lock().unwrap();
        let (dataset_name,) = path.into_inner();

        if let Some(backtest) = jura.new_backtest(dataset_name) {
            Ok(web::Json(InitResponse {
                backtest_id: backtest.0,
                start: backtest.1.start,
                frequency: backtest.1.frequency,
            }))
        } else {
            Err(JuraV1Error::UnknownDataset)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct InfoResponse {
        pub version: String,
        pub dataset: String,
    }

    #[get("/backtest/{backtest_id}/info")]
    pub async fn info(
        app: web::Data<JuraState>,
        path: Path<(BacktestId,)>,
    ) -> Result<web::Json<InfoResponse>, JuraV1Error> {
        let mut jura = app.lock().unwrap();
        let (backtest_id,) = path.into_inner();

        if let Some(state) = jura.exchanges.get_mut(&backtest_id) {
            let info = state.exchange.info();
            Ok(web::Json(InfoResponse {
                version: info.version,
                dataset: info.dataset,
            }))
        } else {
            Err(JuraV1Error::UnknownBacktest)
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};

    use crate::exchange::jura_v1::Order;
    use crate::input::penelope::Penelope;
    use super::AppState;
    use super::jurav1_server::*;

    use std::sync::Mutex;

    #[actix_web::test]
    async fn test_single_trade_loop() {
        let jura = Penelope::random(100);
        let dataset_name = "fake";
        let state = AppState::single(dataset_name, jura);

        let app_state = Mutex::new(state);
        let jura_state = web::Data::new(app_state);

        let app = test::init_service(
            App::new()
                .app_data(jura_state)
                .service(info)
                .service(init)
                .service(fetch_quotes)
                .service(tick)
                .service(insert_order)
                .service(delete_order),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(format!("/init/{dataset_name}").as_str())
            .to_request();
        let resp: InitResponse = test::call_and_read_body_json(&app, req).await;

        let backtest_id = resp.backtest_id;

        let req1 = test::TestRequest::get()
            .uri(format!("/backtest/{backtest_id}/fetch_quotes").as_str())
            .to_request();
        let _resp1: FetchQuotesResponse = test::call_and_read_body_json(&app, req1).await;

        let req2 = test::TestRequest::get()
            .uri(format!("/backtest/{backtest_id}/tick").as_str())
            .to_request();
        let _resp2: TickResponse = test::call_and_read_body_json(&app, req2).await;

        let req3 = test::TestRequest::post()
            .set_json(InsertOrderRequest {
                order: Order::market_buy(0, "100.0", "97.00"),
            })
            .uri(format!("/backtest/{backtest_id}/insert_order").as_str())
            .to_request();
        test::call_and_read_body(&app, req3).await;

        let req4 = test::TestRequest::get()
            .uri(format!("/backtest/{backtest_id}/tick").as_str())
            .to_request();
        let resp4: TickResponse = test::call_and_read_body_json(&app, req4).await;

        assert!(resp4.executed_trades.len() == 1);
        assert!(resp4.executed_trades.get(0).unwrap().coin == "0")
    }
}