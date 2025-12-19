use std::{collections::HashMap, hash::Hash};

use ibapi::{
    Client,
    accounts::types::AccountId,
    market_data::historical::{Duration, WhatToShow},
    prelude::{AccountUpdate, HistoricalBarSize, TradingHours},
};
use time::macros::datetime;

struct Connector {
    ib: Option<Client>,
}

pub trait ConnectorTrait {
    fn new() -> Self;
    async fn connect(&self, address: &str, port: u16, client_id: i32) -> bool;
    fn is_connected(&self) -> bool;
    fn disconnect(&mut self);
    async fn get_account_values(&self) -> Option<Vec<String>>;
    async fn get_positions(&self) -> Option<Vec<String>>;
    async fn market_data(&self, ticker: &str) -> Option<f64>;
    async fn get_lod_hod(&self, ticker: &str) -> (f64, f64);
    fn submit_order(&self, order_details: &str) -> bool;
}

impl ConnectorTrait for Connector {
    fn new() -> Self {
        Connector { ib: None }
    }

    async fn connect(&self, address: &str, port: u16, client_id: i32) -> bool {
        match Client::connect(format!("{}:{}", address, port).as_str(), client_id).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn is_connected(&self) -> bool {
        match &self.ib {
            Some(client) => client.is_connected(),
            None => false,
        }
    }

    fn disconnect(&mut self) {
        if let Some(client) = &self.ib {
            self.ib = None;
        }
    }

    async fn get_account_values(&self) -> Option<Vec<String>> {
        let mut accounts;
        let mut results = Vec::new();
        if !self.is_connected() {
            return None;
        } else {
            accounts = self
                .ib
                .as_ref()
                .unwrap()
                .account_updates(&AccountId { 0: "".into() })
                .await
                .unwrap();
        }

        while let Some(update) = accounts.next().await {
            let v = update.unwrap();
            match v {
                AccountUpdate::AccountValue(val) => {
                    println!(
                        "key: {}, value: {}, currency: {}, account: {}",
                        val.key,
                        val.value,
                        val.currency,
                        val.account.clone().unwrap()
                    );
                    results.push(format!(
                        "key: {}, value: {}, currency: {}, account: {}",
                        val.key,
                        val.value,
                        val.currency,
                        val.account.unwrap()
                    ));
                }
                other => println!("Other update: {:?}", other),
            }
        }
        Some(results)
    }

    async fn get_positions(&self) -> Option<Vec<String>> {
        if !self.is_connected() {
            return None;
        }
        let position_subscription = self.ib.as_ref().unwrap().positions().await;
        let results = match position_subscription {
            Ok(mut positions) => {
                let mut res = Vec::new();
                while let Some(position) = positions.next().await {
                    let pos = position.unwrap();
                    match pos {
                        ibapi::prelude::PositionUpdate::Position(pos_value) => {
                            println!(
                                "Account: {}, Contract: {:?}, Position: {}, Avg cost: {}",
                                pos_value.account,
                                pos_value.contract,
                                pos_value.position,
                                pos_value.average_cost
                            );
                            res.push(format!(
                                "Account: {}, Contract: {:?}, Position: {}, Avg cost: {}",
                                pos_value.account,
                                pos_value.contract,
                                pos_value.position,
                                pos_value.average_cost
                            ));
                        }
                        _ => {
                            println!("Other position data: {:?}", pos);
                        }
                    }
                }
                res
            }
            Err(_) => Vec::new(),
        };
        Some(results)
    }

    async fn market_data(&self, ticker: &str) -> Option<f64> {
        //Get market data for a ticker
        //Returns: current_price or None
        let mut subbed_hash = HashMap::new();
        let stock = ibapi::contracts::Contract::stock(ticker);
        let ib = self
            .ib
            .as_ref()
            .unwrap()
            .contract_details(&stock.build())
            .await;
        match ib {
            Ok(details) => {
                for detail in details {
                    if detail.contract.symbol.0 == ticker {
                        let sub = self.ib.as_ref().unwrap().market_data(&detail.contract);
                        let subbed = sub.subscribe().await;
                        subbed_hash.insert(ticker.to_string(), subbed);
                    }
                }
            }
            Err(e) => {
                println!("Error getting contract details: {:?}", e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let mut current_price = None;

        for _ in 0..10 {
            if let Some(subbed) = subbed_hash.get_mut(ticker) {
                if let Some(price) = subbed.as_mut().unwrap().next().await {
                    match price.unwrap() {
                        ibapi::market_data::realtime::TickTypes::Price(p) => {
                            if p.price > 0.0 {
                                current_price = Some(p.price);
                                println!("Current price for {}: {:?}", ticker, current_price);
                                break;
                            }
                        }
                        _ => {
                            println!("Other tick data received");
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        match current_price {
            Some(_price) => {
                if let Some(subbed) = subbed_hash.get_mut(ticker) {
                    subbed.as_mut().unwrap().cancel().await;
                }
                return current_price;
            }
            None => {
                if let Some(subbed) = subbed_hash.get_mut(ticker) {
                    if let Some(price) = subbed.as_mut().unwrap().next().await {
                        match price.unwrap() {
                            ibapi::market_data::realtime::TickTypes::Price(p) => {
                                if p.price > 0.0 {
                                    current_price = Some(p.price);
                                    println!("Current price for {}: {:?}", ticker, current_price);
                                    current_price
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }

    async fn get_lod_hod(&self, ticker: &str) -> (f64, f64) {
        let stock = ibapi::contracts::Contract::stock(ticker);
        let contract = stock.build();
        let interval_end = Some(datetime!(2023-04-11 20:00 UTC));
        let duration = Duration::seconds(1);
        let bar_size = HistoricalBarSize::Min;
        let what_to_show = Some(WhatToShow::Trades);
        let trading_hours = TradingHours::Regular;
        let historical_data = self
            .ib
            .as_ref()
            .unwrap()
            .historical_data(
                &contract,
                interval_end,
                duration,
                bar_size,
                what_to_show,
                trading_hours,
            )
            .await;

        match historical_data {
            Ok(bars) => {
                let mut lod = f64::MAX;
                let mut hod = f64::MIN;

                bars.bars.iter().for_each(|bar| {
                    if bar.low < lod {
                        lod = bar.low;
                    }
                    if bar.high > hod {
                        hod = bar.high;
                    }
                });
                (lod, hod)
            }
            Err(_) => (0.0, 0.0),
        }
    }

    fn submit_order(&self, _order_details: &str) -> bool {
        true
    }
}
