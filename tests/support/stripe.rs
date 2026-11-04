//! A Stripe of one process: remembers what the app asked and answers from what a test set.

use std::sync::{Arc, Mutex};

use tablo_saas::billing::stripe::{
    Call, Gateway, Invoice, NewCheckout, NewCustomer, NewPrice, Page, PortalProduct, Price,
    Recurring, SeatChange, StripeError, Subscription, SubscriptionItem,
};

/// What the fake holds.
#[derive(Default)]
pub struct State {
    pub subscriptions: Vec<Subscription>,
    pub customers: Vec<NewCustomer>,
    pub checkouts: Vec<NewCheckout>,
    pub seat_changes: Vec<SeatChange>,
    pub canceled: Vec<String>,
    pub refreshes: usize,
    pub down: bool,
}

/// The fake, cheap to clone.
#[derive(Clone, Default)]
pub struct FakeStripe(pub Arc<Mutex<State>>);

impl FakeStripe {
    pub fn with<T>(&self, read: impl FnOnce(&mut State) -> T) -> T {
        read(&mut self.0.lock().expect("the fake's lock"))
    }

    /// Sets the customer's one subscription, as Stripe would now report it.
    pub fn subscribe(&self, customer: &str, status: &str, key: &str, seats: i64) {
        self.with(|state| {
            state.subscriptions.retain(|sub| sub.customer != customer);
            state
                .subscriptions
                .push(subscription(customer, status, key, seats));
        });
    }
}

/// A subscription on a price with lookup key `key`.
pub fn subscription(customer: &str, status: &str, key: &str, seats: i64) -> Subscription {
    Subscription {
        id: format!("sub_{customer}"),
        customer: customer.to_string(),
        status: status.to_string(),
        items: Page {
            data: vec![SubscriptionItem {
                id: format!("si_{customer}"),
                quantity: Some(seats),
                current_period_end: 2_000_000_000,
                price: Price {
                    id: format!("price_{key}"),
                    lookup_key: Some(key.to_string()),
                    recurring: Some(Recurring {
                        interval: "month".to_string(),
                    }),
                },
            }],
        },
        cancel_at_period_end: false,
        trial_end: None,
        metadata: std::collections::HashMap::new(),
        created: 1_800_000_000,
    }
}

impl Gateway for FakeStripe {
    fn product_exists(&self, _id: String) -> Call<'_, bool> {
        Box::pin(async { Ok(true) })
    }
    fn create_product(&self, _id: String, _name: String) -> Call<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn price_by_lookup_key(&self, key: String) -> Call<'_, Option<Price>> {
        Box::pin(async move {
            Ok(Some(Price {
                id: format!("price_{key}"),
                lookup_key: Some(key),
                recurring: None,
            }))
        })
    }
    fn create_price(&self, price: NewPrice) -> Call<'_, Price> {
        Box::pin(async move {
            Ok(Price {
                id: format!("price_{}", price.lookup_key),
                lookup_key: Some(price.lookup_key),
                recurring: None,
            })
        })
    }
    fn configure_portal(&self, _products: Vec<PortalProduct>) -> Call<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn create_customer(&self, customer: NewCustomer) -> Call<'_, String> {
        Box::pin(async move {
            let id = format!("cus_{}", customer.team);
            self.with(|state| state.customers.push(customer));
            Ok(id)
        })
    }
    fn create_checkout(&self, checkout: NewCheckout) -> Call<'_, String> {
        Box::pin(async move {
            self.with(|state| state.checkouts.push(checkout));
            Ok("https://checkout.stripe.test/session".to_string())
        })
    }
    fn create_portal_session(&self, _customer: String, _return_url: String) -> Call<'_, String> {
        Box::pin(async { Ok("https://billing.stripe.test/session".to_string()) })
    }
    fn subscription(&self, id: String) -> Call<'_, Subscription> {
        Box::pin(async move {
            self.with(|state| {
                state
                    .subscriptions
                    .iter()
                    .find(|subscription| subscription.id == id)
                    .cloned()
                    .ok_or_else(|| StripeError::Transport("no such subscription".to_string()))
            })
        })
    }
    fn customer_subscriptions(&self, customer: String) -> Call<'_, Vec<Subscription>> {
        Box::pin(async move {
            self.with(|state| {
                if state.down {
                    return Err(StripeError::Transport("down".to_string()));
                }
                state.refreshes += 1;
                Ok(state
                    .subscriptions
                    .iter()
                    .filter(|subscription| subscription.customer == customer)
                    .cloned()
                    .collect())
            })
        })
    }
    fn set_seats(&self, change: SeatChange) -> Call<'_, ()> {
        Box::pin(async move {
            self.with(|state| {
                if let Some(sub) = state
                    .subscriptions
                    .iter_mut()
                    .find(|subscription| subscription.id == change.subscription)
                {
                    sub.items.data[0].quantity = Some(change.seats);
                }
                state.seat_changes.push(change);
            });
            Ok(())
        })
    }
    fn cancel_subscription(&self, id: String) -> Call<'_, ()> {
        Box::pin(async move {
            self.with(|state| state.canceled.push(id));
            Ok(())
        })
    }
    fn invoices(&self, _customer: String) -> Call<'_, Vec<Invoice>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}
