use banish::banish;
use banish::BanishDispatch;

#[derive(BanishDispatch)]
enum OrderStage {
    Validate,
    ApplyDiscounts,
    Finalize,
}

struct LineItem {
    name: &'static str,
    quantity: u32,
    unit_price_cents: u32,
}

struct Order {
    items: Vec<LineItem>,
    coupon: Option<&'static str>,
}

struct Receipt {
    subtotal_cents: u32,
    discount_cents: u32,
    total_cents: u32,
}

enum OrderResult {
    Ok(Receipt),
    Rejected(&'static str),
}

fn process_order(order: Order, resume_from: OrderStage) -> OrderResult {
    banish! {
        #![dispatch(resume_from)]

        let subtotal_cents: u32 = order.items.iter()
            .map(|i| i.quantity * i.unit_price_cents)
            .sum();
        let mut total_cents: u32 = subtotal_cents;
        let mut discount_cents: u32 = 0;
        let mut rejected: Option<&str> = None;

        @validate
            let mut idx: usize = 0;

            check ? idx < order.items.len() {
                let item = &order.items[idx];
                if item.quantity == 0 || item.unit_price_cents == 0 {
                    rejected = Some(item.name);
                }
                idx += 1;
            }

            route? {
                => @rejected if rejected.is_some();
            }

        @apply_discounts
            loyalty? {
                if subtotal_cents >= 10_000 {
                    discount_cents += subtotal_cents / 10;
                }
            }

            coupon? {
                if let Some(code) = order.coupon {
                    let savings = match code {
                        "SAVE20" => subtotal_cents / 5,
                        "FIVE" => 500_u32.min(subtotal_cents),
                        _ => 0,
                    };
                    discount_cents += savings;
                }
            }

            apply? {
                total_cents = subtotal_cents.saturating_sub(discount_cents);
            }

        @finalize
            done? {
                return OrderResult::Ok(Receipt { subtotal_cents, discount_cents, total_cents });
            }

        #[isolate]
        @rejected
            handle? {
                return OrderResult::Rejected(rejected.unwrap());
            }
    }
}

#[test]
fn dispatch_order_processing() {
    // Full pipeline: validate, apply loyalty and coupon discounts, finalize.
    let result = process_order(
        Order {
            items: vec![
                LineItem { name: "Mechanical Keyboard", quantity: 1, unit_price_cents: 8999 },
                LineItem { name: "USB-C Cable", quantity: 3, unit_price_cents:  999 },
                LineItem { name: "Desk Mat", quantity: 1, unit_price_cents: 2499 },
            ],
            coupon: Some("SAVE20"),
        },
        OrderStage::Validate,
    );
    let OrderResult::Ok(receipt) = result else { panic!("expected Ok"); };
    assert_eq!(receipt.subtotal_cents, 8999 + 2997 + 2499);
    assert!(receipt.discount_cents > 0);
    assert!(receipt.total_cents < receipt.subtotal_cents);

    // Resume from ApplyDiscounts skips validation entirely.
    let result = process_order(
        Order {
            items: vec![
                LineItem { name: "Monitor", quantity: 1, unit_price_cents: 29999 },
                LineItem { name: "HDMI Cable", quantity: 2, unit_price_cents:  1499 },
            ],
            coupon: Some("FIVE"),
        },
        OrderStage::ApplyDiscounts,
    );
    let OrderResult::Ok(receipt) = result else { panic!("expected Ok"); };
    assert_eq!(receipt.subtotal_cents, 29999 + 2998);
    assert_eq!(receipt.discount_cents, 32997 / 10 + 500);
    assert_eq!(receipt.total_cents, receipt.subtotal_cents - receipt.discount_cents);

    // A zero-quantity item is rejected with the offending item name.
    let result = process_order(
        Order {
            items: vec![
                LineItem { name: "Webcam", quantity: 0, unit_price_cents: 5999 },
                LineItem { name: "Headset", quantity: 1, unit_price_cents: 7999 },
            ],
            coupon: None,
        },
        OrderStage::Validate,
    );
    let OrderResult::Rejected(name) = result else { panic!("expected Rejected"); };
    assert_eq!(name, "Webcam");

    // Resume from Finalize skips validation and discounts entirely.
    let result = process_order(
        Order {
            items: vec![
                LineItem { name: "Mousepad", quantity: 1, unit_price_cents: 1999 },
            ],
            coupon: None,
        },
        OrderStage::Finalize,
    );
    let OrderResult::Ok(receipt) = result else { panic!("expected Ok"); };
    assert_eq!(receipt.discount_cents, 0);
    assert_eq!(receipt.total_cents, receipt.subtotal_cents);
}