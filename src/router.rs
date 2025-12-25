use crate::connector::{CONNECTOR, ConnectorTrait};
use axum::{Json, Router, extract::Query, routing::get, routing::post};
use utoipa::OpenApi;

// our router
pub fn app() -> Router {
    Router::new()
        .route("/connect", post(connect))
        .route("/is_connected", get(is_connected))
        .route("/disconnect", post(disconnect))
        .route("/get_account_values", get(get_account_values))
        .route("/get_positions", get(get_positions))
        .route("/market_data", get(get_market_data))
        .route("/get_lod_hod", get(get_lod_hod))
        .route("/order", post(order))
}

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ConnectQuery {
    pub address: String,
    pub port: u16,
    pub client_id: i32,
}

#[utoipa::path(
    get,
    path = "/connect",
    params (
        ("address" = String, Query, description = "The IP address of the IBKR Gateway or TWS"),
        ("port" = u16, Query, description = "The port number to connect to"),
        ("client_id" = i32, Query, description = "The client ID for the connection")
    ),
    tags = ["Connection"],
    responses(
        (status = 200, description = "Connect to IBKR", body = String)
    )
)]
async fn connect(Query(query): Query<ConnectQuery>) -> Json<bool> {
    let ib = CONNECTOR.read().await;
    let result = ib
        .connect(&query.address, query.port, query.client_id)
        .await;
    Json(result)
}

#[utoipa::path(
    get,
    path = "/is_connected",
    tags = ["Connection"],
    responses(
        (status = 200, description = "Check if connected to IBKR")
    )
)]
async fn is_connected() -> Json<bool> {
    let ib = CONNECTOR.read().await;
    let result = ib.is_connected();
    Json(result)
}

#[utoipa::path(
    post,
    path = "/disconnect",
    tags = ["Connection"],
    responses(
        (status = 200, description = "Disconnect from IBKR")
    )
)]
async fn disconnect() {
    let mut ib = CONNECTOR.write().await;
    ib.disconnect();
}

#[utoipa::path(
    get,
    path = "/get_account_values",
    tags = ["Data"],
    responses(
        (status = 200, description = "Get account values from IBKR")
    )
)]
async fn get_account_values() -> Json<Option<Vec<String>>> {
    let ib = CONNECTOR.read().await;
    let account_values = ib.get_account_values().await;
    Json(account_values)
}

#[utoipa::path(
    get,
    path = "/get_positions",
    tags = ["Data"],
    responses(
        (status = 200, description = "Get positions from IBKR")
    )
)]
async fn get_positions() -> Json<Option<Vec<String>>> {
    let ib = CONNECTOR.read().await;
    let positions = ib.get_positions().await;
    Json(positions)
}

#[derive(Deserialize)]
pub struct MarketDataQuery {
    pub ticker: String,
}

#[utoipa::path(
    get,
    path = "/market_data",
    params (
        ("ticker" = String, Query, description = "The ticker symbol for the market data"),
    ),
    tags = ["Data"],
    responses(
        (status = 200, description = "Get market data from IBKR")
    )
)]
async fn get_market_data(Query(query): Query<MarketDataQuery>) -> Json<Option<f64>> {
    let ib = CONNECTOR.read().await;
    let market_data = ib.market_data(&query.ticker).await;
    Json(market_data)
}

#[utoipa::path(
    get,
    path = "/get_lod_hod",
    params (
        ("ticker" = String, Query, description = "The ticker symbol for the market data"),
    ),
    tags = ["Data"],
    responses(
        (status = 200, description = "Get lowest and highest of the day from IBKR")
    )
)]
async fn get_lod_hod(Query(query): Query<MarketDataQuery>) -> Json<(f64, f64)> {
    let ib = CONNECTOR.read().await;
    let lod_hod = ib.get_lod_hod(&query.ticker).await;
    Json(lod_hod)
}

#[utoipa::path(
    post,
    path = "/order",
    params (
        ("ticker" = String, Query, description = "The ticker symbol for the market data"),
        ("qty" = i32, Query, description = "Quantity of shares to order"),
        ("stop_price" = f64, Query, description = "Stop price for the order"),
        ("entry_price" = f64, Query, description = "Entry price for the order"),
        ("action" = String, Query, description = "Action type: BUY or SELL"),
    ),
    tags = ["Data"],
    responses(
        (status = 200, description = "Get market data from IBKR")
    )
)]
async fn order(Query(query): Query<(String, i32, f64, f64, String)>) -> Json<(bool, String)> {
    let ib = CONNECTOR.read().await;
    let market_data = ib
        .submit_order(&query.0, query.1, query.2, query.3, query.4)
        .await;
    Json(market_data)
}

#[derive(OpenApi)]
#[openapi(
    paths(
        connect,
        is_connected,
        disconnect,
        get_account_values,
        get_positions,
        get_market_data,
        get_lod_hod
    ),
    components(
        schemas()
    ),
    tags(
        (name = "connect", description = "Connect to IBKR"),
        (name = "is_connected", description = "Check connection status to IBKR"),
        (name = "disconnect", description = "Disconnect from IBKR"),
        (name = "get_account_values", description = "Get account values from IBKR"),
        (name = "get_positions", description = "Get positions from IBKR"),
        (name = "market_data", description = "Get market data from IBKR"),
        (name = "get_lod_hod", description = "Get lowest and highest of the day from IBKR")
    )
)]
pub struct ApiDoc;
