use std::{collections::HashMap, hash::Hash};

use ibapi::{
    Client,
    accounts::types::AccountId,
    market_data::historical::{Duration, WhatToShow},
    orders::{Action, Order, OrderStatus, PlaceOrder, builder::OrderType},
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
    async fn submit_order(
        &self,
        ticker: &str,
        qty: i32,
        stop_price: f64,
        entry_price: f64,
        action: String,
        order_type: OrderType,
    ) -> (bool, String);
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

    //TODO other order types

    async fn submit_order(
        &self,
        ticker: &str,
        qty: i32,
        stop_price: f64,
        entry_price: f64,
        action: String,
        order_type: OrderType,
    ) -> (bool, String) {
        if self.is_connected() == false {
            return (false, "Not connected to IB Gateway".to_string());
        }
        let contract = ibapi::contracts::Contract::stock(ticker).build();
        let ib = self.ib.as_ref().unwrap().contract_details(&contract).await;

        if order_type == OrderType::Market {
            let mut order = Order::default();
            order.action = match action.as_str() {
                "BUY" => Action::Buy,
                "SELL" => Action::Sell,
                _ => Action::Buy,
            };

            let order_id = self.ib.as_ref().unwrap().next_order_id();

            let mut trade = self
                .ib
                .as_ref()
                .unwrap()
                .place_order(order_id, &contract, &order)
                .await
                .unwrap();

            while let Some(status) = trade.next().await {
                match status {
                    Ok(placeorder) => match placeorder {
                        PlaceOrder::OrderStatus(order_status) => {
                            if order_status.status != "Filled" {
                                return (false, "Market order was not filled.".to_string());
                            }
                            let avg_fill_price = order_status.average_fill_price;
                            let price_diff = if action == "BUY" {
                                avg_fill_price - stop_price
                            } else {
                                stop_price - avg_fill_price
                            };

                            let stop_prices = vec![
                                if action == "BUY" {
                                    (stop_price + price_diff * 2.0 / 3.0 * 100.0).round() / 100.0
                                } else {
                                    (stop_price - price_diff * 2.0 / 3.0 * 100.0).round() / 100.0
                                },
                                if action == "BUY" {
                                    (stop_price + price_diff * 1.0 / 3.0 * 100.0).round() / 100.0
                                } else {
                                    (stop_price - price_diff * 1.0 / 3.0 * 100.0).round() / 100.0
                                },
                                (stop_price * 100.0).round() / 100.0,
                            ];

                            let stop_sizes = vec![qty / 3, qty / 3, qty - 2 * (qty / 3)];

                            for (sp, sq) in stop_prices.iter().zip(stop_sizes.iter()) {
                                order.action = if action == "BUY" {
                                    Action::Buy
                                } else {
                                    Action::Sell
                                };
                                order.order_type = "STOP".to_string();
                                order.total_quantity = *sq as f64;
                                order.aux_price = Some(*sp);

                                let stop_order_id = self.ib.as_ref().unwrap().next_order_id();
                                let _ = self
                                    .ib
                                    .as_ref()
                                    .unwrap()
                                    .place_order(stop_order_id, &contract, &order)
                                    .await;
                            }
                        }
                        _ => {}
                    },
                    Err(e) => {
                        return (false, format!("Error placing order: {:?}", e));
                    }
                }
            }
        }

        //
        //            if order_type == 'Market + 3 Stops':
        //                market_order = MarketOrder(action, qty)
        //                trade = self.ib.placeOrder(contract, market_order)
        //                while trade.isActive():
        //                    self.ib.sleep(1)
        //
        //                if trade.orderStatus.status != 'Filled':
        //                    return False, "Market order was not filled."
        //
        //                avg_fill_price = trade.orderStatus.avgFillPrice
        //                price_diff = avg_fill_price - stop_price if action == 'BUY' else stop_price - avg_fill_price
        //
        //                stop_prices = [
        //                    round(stop_price + price_diff * 2 / 3, 2) if action == 'BUY' else round(stop_price - price_diff * 2 / 3, 2),
        //                    round(stop_price + price_diff * 1 / 3, 2) if action == 'BUY' else round(stop_price - price_diff * 1 / 3, 2),
        //                    round(stop_price, 2)
        //                ]
        //                stop_sizes = [qty // 3, qty // 3, qty - 2 * (qty // 3)]
        //
        //                for sp, sq in zip(stop_prices, stop_sizes):
        //                    stop_order = StopOrder('SELL' if action == 'BUY' else 'BUY', sq, sp, tif='GTC')
        //                    self.ib.placeOrder(contract, stop_order)
        //                    self.ib.sleep(0.5)
        //
        //                return True, f"{action} {qty} shares of {ticker} at ${avg_fill_price:.2f}. 3 stop-loss orders submitted."
        //
        //            elif order_type == '3 Stops Only':
        //                price_diff = entry_price - stop_price if action == 'BUY' else stop_price - entry_price
        //                stop_prices = [
        //                    round(stop_price + price_diff * 2 / 3, 2) if action == 'BUY' else round(stop_price - price_diff * 2 / 3, 2),
        //                    round(stop_price + price_diff * 1 / 3, 2) if action == 'BUY' else round(stop_price - price_diff * 1 / 3, 2),
        //                    round(stop_price, 2)
        //                ]
        //                stop_sizes = [qty // 3, qty // 3, qty - 2 * (qty // 3)]
        //
        //                for sp, sq in zip(stop_prices, stop_sizes):
        //                    stop_order = StopOrder('SELL' if action == 'BUY' else 'BUY', sq, sp, tif='GTC')
        //                    self.ib.placeOrder(contract, stop_order)
        //                    self.ib.sleep(0.5)
        //
        //                return True, f"3 stop-loss orders for {qty} shares of {ticker} submitted."
        //
        //            elif order_type == 'Limit Order':
        //                order = LimitOrder(action, qty, entry_price)
        //                self.ib.placeOrder(contract, order)
        //                return True, f"Limit order to {action} {qty} shares of {ticker} at ${entry_price:.2f} submitted."
        //
        //            elif order_type == 'Stop Order':
        //                order = StopOrder(action, qty, stop_price)
        //                self.ib.placeOrder(contract, order)
        //                return True, f"Stop order to {action} {qty} shares of {ticker} at stop ${stop_price:.2f} submitted."
        //
        //            elif order_type == 'Market + 1 Stop':
        //                market_order = MarketOrder(action, qty)
        //                trade = self.ib.placeOrder(contract, market_order)
        //                while trade.isActive():
        //                    self.ib.sleep(1)
        //
        //                if trade.orderStatus.status != 'Filled':
        //                    return False, "Market order was not filled."
        //
        //                avg_fill_price = trade.orderStatus.avgFillPrice
        //                stop_order = StopOrder('SELL' if action == 'BUY' else 'BUY', qty, stop_price, tif='GTC')
        //                self.ib.placeOrder(contract, stop_order)
        //
        //                return True, f"{action} {qty} shares of {ticker} at ${avg_fill_price:.2f}. 1 stop-loss order submitted at ${stop_price:.2f}."
        //
        //            elif order_type == 'Market + 3 Stops + OCO':
        //                # Place market order
        //                market_order = MarketOrder(action, qty)
        //                trade = self.ib.placeOrder(contract, market_order)
        //                while trade.isActive():
        //                    self.ib.sleep(1)
        //
        //                if trade.orderStatus.status != 'Filled':
        //                    return False, "Market order was not filled."
        //
        //                avg_fill_price = trade.orderStatus.avgFillPrice
        //                price_diff = avg_fill_price - stop_price if action == 'BUY' else stop_price - avg_fill_price
        //
        //                # Calculate the 3 stop prices
        //                stop_prices = [
        //                    round(stop_price + price_diff * 2 / 3, 2) if action == 'BUY' else round(stop_price - price_diff * 2 / 3, 2),
        //                    round(stop_price + price_diff * 1 / 3, 2) if action == 'BUY' else round(stop_price - price_diff * 1 / 3, 2),
        //                    round(stop_price, 2)
        //                ]
        //                # Calculate sizes: 1/3 for OCO, remaining 2/3 divided between the other stops
        //                oco_qty = qty // 3
        //                remaining_qty = qty - oco_qty
        //                stop_sizes = [remaining_qty // 2, remaining_qty - remaining_qty // 2]
        //
        //                # Calculate 2R price (target price for limit sell)
        //                if action == 'BUY':
        //                    target_price = round(avg_fill_price + 2 * price_diff, 2)
        //                    oco_stop_price = stop_prices[0]  # Highest stop (closest to entry)
        //                else:
        //                    target_price = round(avg_fill_price - 2 * price_diff, 2)
        //                    oco_stop_price = stop_prices[0]  # Highest stop
        //
        //                # Create OCO group ID (unique identifier for the OCO pair)
        //                oca_group = f"OCO_{int(time.time() * 1000)}"
        //
        //                # Create limit sell order (target at 2R)
        //                limit_order = LimitOrder('SELL' if action == 'BUY' else 'BUY', oco_qty, target_price, tif='GTC')
        //                limit_order.ocaGroup = oca_group
        //                limit_order.ocaType = 1  # One-Cancels-Other
        //
        //                # Create stop order (highest stop)
        //                oco_stop_order = StopOrder('SELL' if action == 'BUY' else 'BUY', oco_qty, oco_stop_price, tif='GTC')
        //                oco_stop_order.ocaGroup = oca_group
        //                oco_stop_order.ocaType = 1  # One-Cancels-Other
        //
        //                # Place OCO orders
        //                self.ib.placeOrder(contract, limit_order)
        //                self.ib.sleep(0.2)
        //                self.ib.placeOrder(contract, oco_stop_order)
        //                self.ib.sleep(0.5)
        //
        //                # Place the remaining 2 stop orders for the rest of the position
        //                for i in range(1, 3):  # Only the second and third stops
        //                    stop_order = StopOrder('SELL' if action == 'BUY' else 'BUY', stop_sizes[i-1], stop_prices[i], tif='GTC')
        //                    self.ib.placeOrder(contract, stop_order)
        //                    self.ib.sleep(0.5)
        //
        //                return True, f"{action} {qty} shares of {ticker} at ${avg_fill_price:.2f}. OCO (Limit@${target_price:.2f}/Stop@${oco_stop_price:.2f}) + 2 stops submitted."
        //
        //            elif order_type == 'Market Order':
        //                market_order = MarketOrder(action, qty)
        //                trade = self.ib.placeOrder(contract, market_order)
        //                while trade.isActive():
        //                    self.ib.sleep(1)
        //
        //                if trade.orderStatus.status != 'Filled':
        //                    return False, "Market order was not filled."
        //
        //                avg_fill_price = trade.orderStatus.avgFillPrice
        //                return True, f"{action} {qty} shares of {ticker} at market price ${avg_fill_price:.2f} submitted."
        //
        //            else:
        //                return False, "Unknown order type selected."
        //
        //        except Exception as e:
        //            return False, str(e)

        (
            true,
            "Order submission logic not yet implemented.".to_string(),
        )
    }
}
