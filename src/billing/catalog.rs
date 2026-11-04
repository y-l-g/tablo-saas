//! Writing the plan catalog to Stripe.

use super::{
    BillingInterval, CURRENCY, Plan, lookup_key,
    stripe::{Gateway, NewPrice, PortalProduct, StripeError},
};

/// What a catalog sync did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Synced {
    pub products_created: usize,
    pub prices_created: usize,
}

/// Makes Stripe sell what [`Plan::ALL`] says, creating what is missing and changing nothing that
/// exists, then lets the customer portal switch between those prices. Safe to run on every deploy.
pub async fn sync_catalog(stripe: &dyn Gateway) -> Result<Synced, StripeError> {
    let mut synced = Synced::default();
    let mut portal = Vec::new();
    for plan in Plan::paid() {
        let product = plan.product_id();
        if !stripe.product_exists(product.clone()).await? {
            stripe
                .create_product(product.clone(), plan.slug().to_string())
                .await?;
            synced.products_created += 1;
        }
        let mut prices = Vec::new();
        for interval in BillingInterval::BOTH {
            let key = lookup_key(&plan, interval);
            let existing = stripe.price_by_lookup_key(key.clone()).await?;
            let price = if let Some(price) = existing {
                price
            } else {
                synced.prices_created += 1;
                stripe
                    .create_price(NewPrice {
                        product: product.clone(),
                        lookup_key: key,
                        amount: plan.amount(interval),
                        currency: CURRENCY,
                        interval,
                    })
                    .await?
            };
            prices.push(price.id);
        }
        portal.push(PortalProduct { product, prices });
    }
    stripe.configure_portal(portal).await?;
    Ok(synced)
}
