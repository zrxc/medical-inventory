use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppData {
    pub version: u32,
    pub saved_at: String,
    pub products: Vec<Product>,
    pub orders: Vec<Order>,
    pub returns: Vec<ReturnRecord>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: 1,
            saved_at: now_string(),
            products: Vec::new(),
            orders: Vec::new(),
            returns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub purchase_price: f64,
    pub outbound_price: f64,
    pub specification: String,
    pub brand: String,
    pub image_path: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub product_id: Option<Uuid>,
    pub product_name: String,
    #[serde(default)]
    pub item_no: String,
    pub order_time: String,
    pub delivery_date: String,
    pub customer_unit: String,
    pub customer_name: String,
    pub brand: String,
    pub unit: String,
    pub catalog_price: f64,
    pub quantity: f64,
    pub invoice_total: f64,
    pub invoice_status: String,
    pub is_shipped: bool,
    #[serde(default)]
    pub cashback: f64,
    #[serde(default)]
    pub cost_discount: f64,
    pub cost_unit_price: f64,
    pub cost_total: f64,
    pub sale_discount: f64,
    pub invoice_unit_price: f64,
    pub gross_profit: f64,
    pub remark: String,
    pub invoice_no: String,
    pub is_paid: bool,
    pub paid_time: String,
    pub returned_quantity: f64,
    pub created_at: String,
}

impl Order {
    pub fn current_outbound_quantity(&self) -> f64 {
        (self.quantity - self.returned_quantity).max(0.0)
    }

    pub fn current_outbound_amount(&self) -> f64 {
        prorated_amount(
            self.invoice_total,
            self.quantity,
            self.current_outbound_quantity(),
        )
    }

    pub fn refund_amount_for(&self, quantity: f64) -> f64 {
        let quantity = quantity.max(0.0).min(self.current_outbound_quantity());
        prorated_amount(self.invoice_total, self.quantity, quantity)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnRecord {
    pub id: Uuid,
    pub source_order_id: Uuid,
    pub product_id: Option<Uuid>,
    pub product_name: String,
    #[serde(default)]
    pub item_no: String,
    pub return_time: String,
    pub customer_unit: String,
    pub customer_name: String,
    pub brand: String,
    pub unit: String,
    pub catalog_price: f64,
    pub quantity: f64,
    pub invoice_total: f64,
    pub invoice_status: String,
    pub is_shipped: bool,
    #[serde(default)]
    pub cashback: f64,
    #[serde(default)]
    pub cost_discount: f64,
    pub cost_unit_price: f64,
    pub cost_total: f64,
    pub sale_discount: f64,
    pub invoice_unit_price: f64,
    pub gross_profit: f64,
    pub remark: String,
    pub invoice_no: String,
    pub is_paid: bool,
    pub paid_time: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Products,
    Orders,
    Returns,
}

#[derive(Debug, Clone, Default)]
pub struct ProductForm {
    pub name: String,
    pub purchase_price: String,
    pub outbound_price: String,
    pub specification: String,
    pub brand: String,
    pub image_path: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct OrderForm {
    pub product_id: Option<Uuid>,
    pub product_name: String,
    pub item_no: String,
    pub order_time: String,
    pub delivery_date: String,
    pub customer_unit: String,
    pub customer_name: String,
    pub brand: String,
    pub unit: String,
    pub catalog_price: String,
    pub quantity: String,
    pub invoice_total: String,
    pub cashback: String,
    pub cost_discount: String,
    pub cost_unit_price: String,
    pub cost_total: String,
    pub sale_discount: String,
    pub invoice_unit_price: String,
    pub gross_profit: String,
    pub remark: String,
    pub paid_time: String,
}

impl Default for OrderForm {
    fn default() -> Self {
        Self {
            product_id: None,
            product_name: String::new(),
            item_no: String::new(),
            order_time: today_string(),
            delivery_date: today_string(),
            customer_unit: String::new(),
            customer_name: String::new(),
            brand: String::new(),
            unit: "件".into(),
            catalog_price: "0".into(),
            quantity: "1".into(),
            invoice_total: "0".into(),
            cashback: "0".into(),
            cost_discount: "0".into(),
            cost_unit_price: "0".into(),
            cost_total: "0".into(),
            sale_discount: "0".into(),
            invoice_unit_price: "0".into(),
            gross_profit: "0".into(),
            remark: String::new(),
            paid_time: today_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReturnForm {
    pub source_order_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub product_name: String,
    pub item_no: String,
    pub return_time: String,
    pub customer_unit: String,
    pub customer_name: String,
    pub brand: String,
    pub unit: String,
    pub catalog_price: String,
    pub quantity: String,
    pub invoice_total: String,
    pub cashback: String,
    pub cost_discount: String,
    pub cost_unit_price: String,
    pub cost_total: String,
    pub sale_discount: String,
    pub invoice_unit_price: String,
    pub gross_profit: String,
    pub remark: String,
    pub paid_time: String,
}

impl Default for ReturnForm {
    fn default() -> Self {
        Self {
            source_order_id: None,
            product_id: None,
            product_name: String::new(),
            item_no: String::new(),
            return_time: today_string(),
            customer_unit: String::new(),
            customer_name: String::new(),
            brand: String::new(),
            unit: "件".into(),
            catalog_price: "0".into(),
            quantity: "1".into(),
            invoice_total: "0".into(),
            cashback: "0".into(),
            cost_discount: "0".into(),
            cost_unit_price: "0".into(),
            cost_total: "0".into(),
            sale_discount: "0".into(),
            invoice_unit_price: "0".into(),
            gross_profit: "0".into(),
            remark: String::new(),
            paid_time: today_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DashboardFilter {
    pub start_date: String,
    pub end_date: String,
}

impl Default for DashboardFilter {
    fn default() -> Self {
        let today = Local::now().date_naive();
        let start = today - Duration::days(6);
        Self {
            start_date: start.format("%Y-%m-%d").to_string(),
            end_date: today.format("%Y-%m-%d").to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DailyStat {
    pub label: String,
    pub outbound_amount: f64,
    pub return_amount: f64,
}

pub fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn today_string() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn parse_date(input: &str) -> Option<NaiveDate> {
    let input = input.trim();
    [
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%Y/%-m/%-d",
        "%Y.%m.%d",
        "%Y.%-m.%-d",
    ]
    .iter()
    .find_map(|format| NaiveDate::parse_from_str(input, format).ok())
}

pub fn normalize_date(input: &str) -> Option<String> {
    parse_date(input).map(|date| date.format("%Y-%m-%d").to_string())
}

pub fn parse_f64(input: &str) -> Result<f64, String> {
    let trimmed = input.trim();
    let normalized = trimmed
        .chars()
        .filter(|ch| !matches!(ch, ',' | '，' | '¥' | '￥'))
        .collect::<String>();
    let normalized = normalized.trim();
    let (number_text, divisor) = normalized
        .strip_suffix('%')
        .map_or((normalized, 1.0), |value| (value.trim(), 100.0));
    match number_text.parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(value / divisor),
        _ => Err(format!("数值格式不正确: {trimmed}")),
    }
}

pub fn format_money(value: f64) -> String {
    format!("{value:.2}")
}

pub fn prorated_amount(total: f64, original_quantity: f64, quantity: f64) -> f64 {
    if original_quantity <= 0.0 {
        return 0.0;
    }
    total * (quantity / original_quantity)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(quantity: f64, returned_quantity: f64, invoice_total: f64) -> Order {
        Order {
            id: Uuid::new_v4(),
            product_id: None,
            product_name: "测试产品".into(),
            item_no: String::new(),
            order_time: "2026-05-30".into(),
            delivery_date: "2026-05-30".into(),
            customer_unit: "测试单位".into(),
            customer_name: "测试人".into(),
            brand: String::new(),
            unit: "件".into(),
            catalog_price: 0.0,
            quantity,
            invoice_total,
            invoice_status: String::new(),
            is_shipped: true,
            cashback: 0.0,
            cost_discount: 0.0,
            cost_unit_price: 0.0,
            cost_total: 0.0,
            sale_discount: 0.0,
            invoice_unit_price: 0.0,
            gross_profit: 0.0,
            remark: String::new(),
            invoice_no: String::new(),
            is_paid: true,
            paid_time: String::new(),
            returned_quantity,
            created_at: "2026-05-30 00:00:00".into(),
        }
    }

    #[test]
    fn refund_amount_matches_return_quantity_ratio() {
        let order = order(10.0, 0.0, 1200.0);

        assert_eq!(order.refund_amount_for(3.0), 360.0);
    }

    #[test]
    fn refund_amount_is_capped_by_available_outbound_quantity() {
        let order = order(10.0, 8.0, 1200.0);

        assert_eq!(order.refund_amount_for(5.0), 240.0);
    }

    #[test]
    fn parse_f64_rejects_non_finite_numbers() {
        assert!(parse_f64("NaN").is_err());
        assert!(parse_f64("inf").is_err());
    }

    #[test]
    fn parse_f64_accepts_excel_style_money_text() {
        assert_eq!(parse_f64("¥1,234.50").unwrap(), 1234.5);
        assert_eq!(parse_f64("￥1，234.50").unwrap(), 1234.5);
    }

    #[test]
    fn parse_f64_accepts_percent_text() {
        assert_eq!(parse_f64("80%").unwrap(), 0.8);
        assert_eq!(parse_f64("12.5%").unwrap(), 0.125);
    }

    #[test]
    fn parse_date_accepts_excel_style_separators_and_normalizes() {
        assert_eq!(normalize_date("2026/5/30"), Some("2026-05-30".into()));
        assert_eq!(normalize_date("2026.05.30"), Some("2026-05-30".into()));
        assert_eq!(normalize_date("2026-05-30"), Some("2026-05-30".into()));
    }
}
