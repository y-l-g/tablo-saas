//! The HTTP client for Stripe's API.

use std::{fmt::Display, time::Duration};

use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};

use super::{
    Call, Gateway, Invoice, NewCheckout, NewCustomer, NewPrice, Page, PortalProduct, Price,
    SeatChange, StripeError, Subscription,
};

/// The API version every request pins, so an account's default cannot change a response's shape
/// under a running app. Bump it deliberately, reading Stripe's changelog.
const API_VERSION: &str = "2025-10-29.clover";

/// How often a request that failed for a passing reason is sent again.
const RETRIES: u32 = 2;

/// Stripe's API over HTTPS.
pub struct StripeClient {
    http: Client,
    key: String,
    base: String,
}

/// A request's form fields, in Stripe's `a[b][0]=c` spelling.
type Form = Vec<(String, String)>;

fn field(form: &mut Form, name: &str, value: impl Display) {
    form.push((name.to_string(), value.to_string()));
}

#[derive(Deserialize)]
struct Failure {
    error: FailureBody,
}

#[derive(Deserialize)]
struct FailureBody {
    code: Option<String>,
    message: Option<String>,
}

/// An object known by its id alone.
#[derive(Deserialize)]
struct Identified {
    id: String,
}

/// A hosted page.
#[derive(Deserialize)]
struct Hosted {
    url: Option<String>,
}

impl StripeClient {
    pub fn new(key: &str, base: &str) -> anyhow::Result<Self> {
        Ok(Self {
            http: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                .build()?,
            key: key.to_string(),
            base: base.trim_end_matches('/').to_string(),
        })
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.key)
            .header("Stripe-Version", API_VERSION)
    }

    /// Sends a request built by `build`, again after a passing failure. A write carries one
    /// idempotency key through its retries, so Stripe applies it once.
    async fn send(
        &self,
        build: impl Fn() -> RequestBuilder + Send + Sync,
    ) -> Result<Response, StripeError> {
        let mut attempt = 0;
        loop {
            let error = match build().send().await {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response) => failure(response).await,
                Err(error) => StripeError::Transport(error.to_string()),
            };
            if attempt == RETRIES || !error.is_transient() {
                return Err(error);
            }
            attempt += 1;
            tokio::time::sleep(Duration::from_millis(250 * u64::from(attempt.pow(2)))).await;
        }
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, StripeError> {
        let response = self
            .send(|| self.request(Method::GET, path).query(query))
            .await?;
        decode(response).await
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        form: &Form,
        idempotency_key: &str,
    ) -> Result<T, StripeError> {
        let response = self
            .send(|| {
                self.request(Method::POST, path)
                    .header("Idempotency-Key", idempotency_key)
                    .form(form)
            })
            .await?;
        decode(response).await
    }
}

async fn failure(response: Response) -> StripeError {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    match serde_json::from_str::<Failure>(&body) {
        Ok(Failure { error }) => StripeError::Api {
            status,
            code: error.code,
            message: error.message.unwrap_or_default(),
        },
        Err(_) => StripeError::Api {
            status,
            code: None,
            message: body.chars().take(200).collect(),
        },
    }
}

async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, StripeError> {
    response
        .json()
        .await
        .map_err(|error| StripeError::Decode(error.to_string()))
}

fn fresh_key() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn hosted(page: Hosted) -> Result<String, StripeError> {
    page.url
        .ok_or_else(|| StripeError::Decode("the session carries no url".to_string()))
}

impl Gateway for StripeClient {
    fn product_exists(&self, id: String) -> Call<'_, bool> {
        Box::pin(async move {
            match self
                .get::<Identified>(&format!("/v1/products/{id}"), &[])
                .await
            {
                Ok(_) => Ok(true),
                Err(StripeError::Api { status, .. }) if status == StatusCode::NOT_FOUND => {
                    Ok(false)
                }
                Err(error) => Err(error),
            }
        })
    }

    fn create_product(&self, id: String, name: String) -> Call<'_, ()> {
        Box::pin(async move {
            let mut form = Form::new();
            field(&mut form, "id", &id);
            field(&mut form, "name", name);
            self.post::<Identified>("/v1/products", &form, &format!("product-{id}"))
                .await?;
            Ok(())
        })
    }

    fn price_by_lookup_key(&self, key: String) -> Call<'_, Option<Price>> {
        Box::pin(async move {
            let page: Page<Price> = self
                .get(
                    "/v1/prices",
                    &[("lookup_keys[]", key.as_str()), ("active", "true")],
                )
                .await?;
            Ok(page.data.into_iter().next())
        })
    }

    fn create_price(&self, price: NewPrice) -> Call<'_, Price> {
        Box::pin(async move {
            let mut form = Form::new();
            field(&mut form, "product", price.product);
            field(&mut form, "lookup_key", &price.lookup_key);
            field(&mut form, "unit_amount", price.amount);
            field(&mut form, "currency", price.currency);
            field(&mut form, "recurring[interval]", price.interval);
            field(&mut form, "tax_behavior", "exclusive");
            self.post("/v1/prices", &form, &format!("price-{}", price.lookup_key))
                .await
        })
    }

    fn configure_portal(&self, products: Vec<PortalProduct>) -> Call<'_, ()> {
        Box::pin(async move {
            let mut form = Form::new();
            field(&mut form, "features[invoice_history][enabled]", true);
            field(&mut form, "features[payment_method_update][enabled]", true);
            field(&mut form, "features[customer_update][enabled]", true);
            field(
                &mut form,
                "features[customer_update][allowed_updates][0]",
                "address",
            );
            field(
                &mut form,
                "features[customer_update][allowed_updates][1]",
                "tax_id",
            );
            field(&mut form, "features[subscription_cancel][enabled]", true);
            field(
                &mut form,
                "features[subscription_cancel][mode]",
                "at_period_end",
            );
            field(&mut form, "features[subscription_update][enabled]", true);
            field(
                &mut form,
                "features[subscription_update][proration_behavior]",
                "create_prorations",
            );
            // Seats follow the member list, so the portal changes the plan and never the quantity.
            field(
                &mut form,
                "features[subscription_update][default_allowed_updates][0]",
                "price",
            );
            for (index, product) in products.iter().enumerate() {
                let base = format!("features[subscription_update][products][{index}]");
                field(&mut form, &format!("{base}[product]"), &product.product);
                for (position, price) in product.prices.iter().enumerate() {
                    field(&mut form, &format!("{base}[prices][{position}]"), price);
                }
            }
            let existing: Page<Identified> = self
                .get(
                    "/v1/billing_portal/configurations",
                    &[("is_default", "true"), ("limit", "1")],
                )
                .await?;
            let path = existing.data.first().map_or_else(
                || "/v1/billing_portal/configurations".to_string(),
                |configuration| format!("/v1/billing_portal/configurations/{}", configuration.id),
            );
            self.post::<Identified>(&path, &form, &fresh_key()).await?;
            Ok(())
        })
    }

    fn create_customer(&self, customer: NewCustomer) -> Call<'_, String> {
        Box::pin(async move {
            let mut form = Form::new();
            field(&mut form, "name", customer.name);
            field(&mut form, "email", customer.email);
            field(&mut form, "metadata[team_id]", &customer.team);
            // The team fixes the key, so two people pressing Upgrade make one customer.
            let created: Identified = self
                .post(
                    "/v1/customers",
                    &form,
                    &format!("customer-{}", customer.team),
                )
                .await?;
            Ok(created.id)
        })
    }

    fn create_checkout(&self, checkout: NewCheckout) -> Call<'_, String> {
        Box::pin(async move {
            let mut form = Form::new();
            field(&mut form, "mode", "subscription");
            field(&mut form, "customer", checkout.customer);
            field(&mut form, "client_reference_id", &checkout.team);
            field(&mut form, "line_items[0][price]", checkout.price);
            field(&mut form, "line_items[0][quantity]", checkout.seats);
            field(&mut form, "success_url", checkout.success_url);
            field(&mut form, "cancel_url", checkout.cancel_url);
            field(&mut form, "allow_promotion_codes", true);
            field(
                &mut form,
                "subscription_data[metadata][team_id]",
                &checkout.team,
            );
            if let Some(days) = checkout.trial_days {
                field(&mut form, "subscription_data[trial_period_days]", days);
            }
            if checkout.automatic_tax {
                field(&mut form, "automatic_tax[enabled]", true);
                field(&mut form, "customer_update[address]", "auto");
                field(&mut form, "customer_update[name]", "auto");
                field(&mut form, "tax_id_collection[enabled]", true);
            }
            let session: Hosted = self
                .post("/v1/checkout/sessions", &form, &fresh_key())
                .await?;
            hosted(session)
        })
    }

    fn create_portal_session(&self, customer: String, return_url: String) -> Call<'_, String> {
        Box::pin(async move {
            let mut form = Form::new();
            field(&mut form, "customer", customer);
            field(&mut form, "return_url", return_url);
            let session: Hosted = self
                .post("/v1/billing_portal/sessions", &form, &fresh_key())
                .await?;
            hosted(session)
        })
    }

    fn subscription(&self, id: String) -> Call<'_, Subscription> {
        Box::pin(async move { self.get(&format!("/v1/subscriptions/{id}"), &[]).await })
    }

    fn customer_subscriptions(&self, customer: String) -> Call<'_, Vec<Subscription>> {
        Box::pin(async move {
            let page: Page<Subscription> = self
                .get(
                    "/v1/subscriptions",
                    &[
                        ("customer", customer.as_str()),
                        ("status", "all"),
                        ("limit", "10"),
                    ],
                )
                .await?;
            Ok(page.data)
        })
    }

    fn set_seats(&self, change: SeatChange) -> Call<'_, ()> {
        Box::pin(async move {
            let mut form = Form::new();
            field(&mut form, "quantity", change.seats);
            field(&mut form, "proration_behavior", "create_prorations");
            self.post::<Identified>(
                &format!("/v1/subscription_items/{}", change.item),
                &form,
                &format!(
                    "seats-{}-{}-{}",
                    change.subscription,
                    change.seats,
                    uuid::Uuid::now_v7()
                ),
            )
            .await?;
            Ok(())
        })
    }

    fn cancel_subscription(&self, id: String) -> Call<'_, ()> {
        Box::pin(async move {
            let path = format!("/v1/subscriptions/{id}");
            let response = self.send(|| self.request(Method::DELETE, &path)).await;
            match response {
                Ok(_) => Ok(()),
                // Already gone is what the caller wanted.
                Err(StripeError::Api { status, .. }) if status == StatusCode::NOT_FOUND => Ok(()),
                Err(error) => Err(error),
            }
        })
    }

    fn invoices(&self, customer: String) -> Call<'_, Vec<Invoice>> {
        Box::pin(async move {
            let page: Page<Invoice> = self
                .get(
                    "/v1/invoices",
                    &[("customer", customer.as_str()), ("limit", "12")],
                )
                .await?;
            Ok(page.data)
        })
    }
}
