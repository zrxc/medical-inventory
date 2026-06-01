use std::path::Path;

use chrono::{Datelike, Duration, Local, NaiveDate};
use eframe::egui::{
    self, Align2, Color32, ComboBox, FontId, Frame, Grid, RichText, ScrollArea, Sense, Stroke,
    TextEdit, Ui, Vec2,
};
use uuid::Uuid;

use crate::{
    models::{
        AppData, DailyStat, DashboardFilter, Order, OrderForm, Page, Product, ProductForm,
        ReturnForm, ReturnRecord, format_money, normalize_date, now_string, parse_date, parse_f64,
        today_string,
    },
    storage::Storage,
};

pub struct MedicalInventoryApp {
    storage: Storage,
    data: AppData,
    page: Page,
    product_form: ProductForm,
    order_form: OrderForm,
    return_form: ReturnForm,
    editing_product_id: Option<Uuid>,
    editing_order_id: Option<Uuid>,
    editing_return_id: Option<Uuid>,
    product_query: String,
    order_query: String,
    return_query: String,
    product_import_path: String,
    order_import_path: String,
    return_import_path: String,
    dashboard_filter: DashboardFilter,
    status_message: String,
}

impl MedicalInventoryApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let storage = Storage::new();
        let outcome = storage.load();

        Self {
            storage,
            data: outcome.data,
            page: Page::Dashboard,
            product_form: ProductForm::default(),
            order_form: OrderForm::default(),
            return_form: ReturnForm::default(),
            editing_product_id: None,
            editing_order_id: None,
            editing_return_id: None,
            product_query: String::new(),
            order_query: String::new(),
            return_query: String::new(),
            product_import_path: String::new(),
            order_import_path: String::new(),
            return_import_path: String::new(),
            dashboard_filter: DashboardFilter::default(),
            status_message: outcome.notice,
        }
    }

    fn save_data(&mut self, action: &str) {
        self.data.saved_at = now_string();
        self.status_message = match self.storage.save(&self.data) {
            Ok(()) => format!(
                "{action}，已保存。数据文件: {}，备份目录: {}",
                self.storage.data_file().display(),
                self.storage.backup_dir().display()
            ),
            Err(err) => format!("{action}，但保存失败: {err}"),
        };
    }

    fn save_product(&mut self) {
        let name = self.product_form.name.trim();
        if name.is_empty() {
            self.status_message = "产品名称不能为空".into();
            return;
        }
        let specification = self.product_form.specification.trim();
        let brand = self.product_form.brand.trim();

        if self.data.products.iter().any(|product| {
            Some(product.id) != self.editing_product_id
                && product_identity_matches(product, name, brand, specification)
        }) {
            self.status_message = "相同名称、品牌、规格/型号的产品已存在".into();
            return;
        }

        let purchase_price = match parse_f64(&self.product_form.purchase_price) {
            Ok(value) => value,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        let outbound_price = match parse_f64(&self.product_form.outbound_price) {
            Ok(value) => value,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        if let Err(err) = validate_non_negative_fields(&[
            ("进货金额", purchase_price),
            ("出库金额", outbound_price),
        ]) {
            self.status_message = err;
            return;
        }

        let product = Product {
            id: self.editing_product_id.unwrap_or_else(Uuid::new_v4),
            name: name.to_string(),
            purchase_price,
            outbound_price,
            specification: specification.to_string(),
            brand: brand.to_string(),
            image_path: self.product_form.image_path.trim().to_string(),
            description: self.product_form.description.trim().to_string(),
            created_at: self
                .editing_product_id
                .and_then(|id| {
                    self.data
                        .products
                        .iter()
                        .find(|product| product.id == id)
                        .map(|product| product.created_at.clone())
                })
                .unwrap_or_else(now_string),
        };

        if let Some(product_id) = self.editing_product_id {
            if let Some(existing) = self
                .data
                .products
                .iter_mut()
                .find(|item| item.id == product_id)
            {
                *existing = product;
                self.product_form = ProductForm::default();
                self.editing_product_id = None;
                self.save_data("产品已更新");
            } else {
                self.status_message = "要编辑的产品不存在".into();
            }
            return;
        }

        self.data.products.push(product);
        self.product_form = ProductForm::default();
        self.save_data("产品已新增");
    }

    fn edit_product(&mut self, product_id: Uuid) {
        let Some(product) = self.data.products.iter().find(|item| item.id == product_id) else {
            self.status_message = "要编辑的产品不存在".into();
            return;
        };

        self.product_form = ProductForm {
            name: product.name.clone(),
            purchase_price: format_money(product.purchase_price),
            outbound_price: format_money(product.outbound_price),
            specification: product.specification.clone(),
            brand: product.brand.clone(),
            image_path: product.image_path.clone(),
            description: product.description.clone(),
        };
        self.editing_product_id = Some(product.id);
        self.status_message = format!("正在编辑产品“{}”", product.name);
    }

    fn cancel_product_edit(&mut self) {
        self.product_form = ProductForm::default();
        self.editing_product_id = None;
        self.status_message = "已取消产品编辑".into();
    }

    fn clear_product_form(&mut self) {
        self.product_form = ProductForm::default();
        self.status_message = "产品表单已清空".into();
    }

    fn import_products_csv(&mut self) {
        let path_text = self.product_import_path.trim().trim_matches('"');
        if path_text.is_empty() {
            self.status_message = "请先填写产品CSV文件路径".into();
            return;
        }

        let imported_products = match self.storage.read_products_csv(Path::new(path_text)) {
            Ok(products) => products,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        if imported_products.is_empty() {
            self.status_message = "产品CSV没有可导入的产品".into();
            return;
        }

        let summary = merge_imported_products(&mut self.data.products, imported_products);
        if summary.imported > 0 {
            self.save_data(&format!(
                "产品CSV已导入 {} 条，跳过重复 {} 条",
                summary.imported, summary.skipped_duplicates
            ));
        } else {
            self.status_message = format!(
                "产品CSV没有新增记录，跳过重复 {} 条",
                summary.skipped_duplicates
            );
        }
    }

    fn import_orders_csv(&mut self) {
        let path_text = self.order_import_path.trim().trim_matches('"');
        if path_text.is_empty() {
            self.status_message = "请先填写订单CSV文件路径".into();
            return;
        }

        let imported_orders = match self
            .storage
            .read_orders_csv(Path::new(path_text), &self.data.products)
        {
            Ok(orders) => orders,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        if imported_orders.is_empty() {
            self.status_message = "订单CSV没有可导入的订单".into();
            return;
        }

        let summary = merge_imported_orders(&mut self.data.orders, imported_orders);
        if summary.imported > 0 {
            self.save_data(&format!(
                "订单CSV已导入 {} 条，跳过重复 {} 条",
                summary.imported, summary.skipped_duplicates
            ));
        } else {
            self.status_message = format!(
                "订单CSV没有新增记录，跳过重复 {} 条",
                summary.skipped_duplicates
            );
        }
    }

    fn import_returns_csv(&mut self) {
        let path_text = self.return_import_path.trim().trim_matches('"');
        if path_text.is_empty() {
            self.status_message = "请先填写退货CSV文件路径".into();
            return;
        }

        let imported_returns = match self.storage.read_returns_csv(
            Path::new(path_text),
            &self.data.orders,
            &self.data.products,
        ) {
            Ok(records) => records,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        if imported_returns.is_empty() {
            self.status_message = "退货CSV没有可导入的记录".into();
            return;
        }

        let mut orders = self.data.orders.clone();
        let mut returns = self.data.returns.clone();
        let summary = match merge_imported_returns(&mut orders, &mut returns, imported_returns) {
            Ok(summary) => summary,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        if summary.imported > 0 {
            self.data.orders = orders;
            self.data.returns = returns;
            self.save_data(&format!(
                "退货CSV已导入 {} 条，跳过重复 {} 条，并已回写订单出库数量",
                summary.imported, summary.skipped_duplicates
            ));
        } else {
            self.status_message = format!(
                "退货CSV没有新增记录，跳过重复 {} 条",
                summary.skipped_duplicates
            );
        }
    }

    fn delete_product(&mut self, product_id: Uuid) {
        let referenced_by_order = self
            .data
            .orders
            .iter()
            .any(|order| order.product_id == Some(product_id));
        let referenced_by_return = self
            .data
            .returns
            .iter()
            .any(|record| record.product_id == Some(product_id));

        if referenced_by_order || referenced_by_return {
            self.status_message = "该产品已被订单或退货记录引用，不能直接删除".into();
            return;
        }

        let before = self.data.products.len();
        self.data
            .products
            .retain(|product| product.id != product_id);
        if self.data.products.len() == before {
            self.status_message = "要删除的产品不存在".into();
            return;
        }

        if self.editing_product_id == Some(product_id) {
            self.cancel_product_edit();
        }
        self.save_data("产品已删除");
    }

    fn save_order(&mut self) {
        let product_name = self.order_form.product_name.trim();
        if product_name.is_empty() {
            self.status_message = "下单前请填写产品名称".into();
            return;
        }

        let order_time = self.order_form.order_time.trim();
        let Some(order_date) = parse_date(order_time) else {
            self.status_message = "订购时间格式不正确，请使用 YYYY-MM-DD 或 YYYY/M/D".into();
            return;
        };

        let delivery_date = self.order_form.delivery_date.trim();
        let Some(delivery_date) = parse_date(delivery_date) else {
            self.status_message = "出库日期格式不正确，请使用 YYYY-MM-DD 或 YYYY/M/D".into();
            return;
        };
        if !delivery_date_is_valid_for_order(order_date, delivery_date) {
            self.status_message = "出库日期不能早于订购日期".into();
            return;
        }

        let (order_id, returned_quantity, created_at) = if let Some(order_id) =
            self.editing_order_id
        {
            let Some(existing) = self.data.orders.iter().find(|order| order.id == order_id) else {
                self.status_message = "要编辑的订单不存在".into();
                return;
            };
            (
                existing.id,
                existing.returned_quantity,
                existing.created_at.clone(),
            )
        } else {
            (Uuid::new_v4(), 0.0, now_string())
        };

        let order = match self.build_order(order_id, returned_quantity, created_at) {
            Ok(order) => order,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };

        if order.quantity <= 0.0 {
            self.status_message = "订单数量必须大于 0".into();
            return;
        }
        if order.quantity < order.returned_quantity {
            self.status_message = format!(
                "订单数量不能小于已退数量 {}",
                format_money(order.returned_quantity)
            );
            return;
        }
        if let Err(err) = validate_non_negative_fields(&[
            ("目录价", order.catalog_price),
            ("销售总价", order.invoice_total),
            ("返现", order.cashback),
            ("成本折扣", order.cost_discount),
            ("成本单价", order.cost_unit_price),
            ("成本总价", order.cost_total),
            ("售价折扣", order.sale_discount),
            ("销售单价", order.invoice_unit_price),
        ]) {
            self.status_message = err;
            return;
        }

        if let Some(order_id) = self.editing_order_id {
            if let Some(existing) = self.data.orders.iter_mut().find(|item| item.id == order_id) {
                *existing = order;
                self.order_form = OrderForm::default();
                self.editing_order_id = None;
                self.save_data("订单已更新");
            } else {
                self.status_message = "要编辑的订单不存在".into();
            }
            return;
        }

        self.data.orders.push(order);
        self.order_form = OrderForm::default();
        self.save_data("订单已新增");
    }

    fn build_order(
        &self,
        id: Uuid,
        returned_quantity: f64,
        created_at: String,
    ) -> Result<Order, String> {
        Ok(Order {
            id,
            product_id: self.order_form.product_id,
            product_name: self.order_form.product_name.trim().to_string(),
            item_no: self.order_form.item_no.trim().to_string(),
            order_time: normalize_date(&self.order_form.order_time)
                .unwrap_or_else(|| self.order_form.order_time.trim().to_string()),
            delivery_date: normalize_date(&self.order_form.delivery_date)
                .unwrap_or_else(|| self.order_form.delivery_date.trim().to_string()),
            customer_unit: self.order_form.customer_unit.trim().to_string(),
            customer_name: self.order_form.customer_name.trim().to_string(),
            brand: self.order_form.brand.trim().to_string(),
            unit: self.order_form.unit.trim().to_string(),
            catalog_price: parse_f64(&self.order_form.catalog_price)?,
            quantity: parse_f64(&self.order_form.quantity)?,
            invoice_total: parse_f64(&self.order_form.invoice_total)?,
            invoice_status: String::new(),
            is_shipped: true,
            cashback: parse_f64(&self.order_form.cashback)?,
            cost_discount: parse_f64(&self.order_form.cost_discount)?,
            cost_unit_price: parse_f64(&self.order_form.cost_unit_price)?,
            cost_total: parse_f64(&self.order_form.cost_total)?,
            sale_discount: parse_f64(&self.order_form.sale_discount)?,
            invoice_unit_price: parse_f64(&self.order_form.invoice_unit_price)?,
            gross_profit: parse_f64(&self.order_form.gross_profit)?,
            remark: self.order_form.remark.trim().to_string(),
            invoice_no: String::new(),
            is_paid: true,
            paid_time: normalized_date_text(&self.order_form.paid_time),
            returned_quantity,
            created_at,
        })
    }

    fn recalculate_order_amounts(&mut self) {
        match calculate_form_amounts(
            &self.order_form.quantity,
            &self.order_form.invoice_unit_price,
            &self.order_form.cost_unit_price,
            &self.order_form.cashback,
        ) {
            Ok(amounts) => {
                self.order_form.invoice_total = format_money(amounts.invoice_total);
                self.order_form.cost_total = format_money(amounts.cost_total);
                self.order_form.gross_profit = format_money(amounts.gross_profit);
                self.status_message = "订单金额已重算".into();
            }
            Err(err) => {
                self.status_message = err;
            }
        }
    }

    fn edit_order(&mut self, order_id: Uuid) {
        let Some(order) = self.data.orders.iter().find(|item| item.id == order_id) else {
            self.status_message = "要编辑的订单不存在".into();
            return;
        };

        self.order_form = OrderForm {
            product_id: order.product_id,
            product_name: order.product_name.clone(),
            item_no: order.item_no.clone(),
            order_time: order.order_time.clone(),
            delivery_date: order.delivery_date.clone(),
            customer_unit: order.customer_unit.clone(),
            customer_name: order.customer_name.clone(),
            brand: order.brand.clone(),
            unit: order.unit.clone(),
            catalog_price: format_money(order.catalog_price),
            quantity: format_money(order.quantity),
            invoice_total: format_money(order.invoice_total),
            cashback: format_money(order.cashback),
            cost_discount: format_money(order.cost_discount),
            cost_unit_price: format_money(order.cost_unit_price),
            cost_total: format_money(order.cost_total),
            sale_discount: format_money(order.sale_discount),
            invoice_unit_price: format_money(order.invoice_unit_price),
            gross_profit: format_money(order.gross_profit),
            remark: order.remark.clone(),
            paid_time: order.paid_time.clone(),
        };
        self.editing_order_id = Some(order.id);
        self.page = Page::Orders;
        self.status_message = format!(
            "正在编辑订单“{} / {}”",
            order.product_name, order.customer_unit
        );
    }

    fn cancel_order_edit(&mut self) {
        self.order_form = OrderForm::default();
        self.editing_order_id = None;
        self.status_message = "已取消订单编辑".into();
    }

    fn clear_order_form(&mut self) {
        self.order_form = OrderForm::default();
        self.status_message = "订单表单已清空".into();
    }

    fn delete_order(&mut self, order_id: Uuid) {
        if self
            .data
            .returns
            .iter()
            .any(|record| record.source_order_id == order_id)
        {
            self.status_message = "该订单已有退货记录，不能直接删除".into();
            return;
        }

        let before = self.data.orders.len();
        self.data.orders.retain(|order| order.id != order_id);
        if self.data.orders.len() == before {
            self.status_message = "要删除的订单不存在".into();
            return;
        }

        if self.editing_order_id == Some(order_id) {
            self.cancel_order_edit();
        }
        self.save_data("订单已删除");
    }

    fn save_return(&mut self) {
        let Some(order_id) = self.return_form.source_order_id else {
            self.status_message = "退货必须关联一个订单".into();
            return;
        };

        let quantity = match parse_f64(&self.return_form.quantity) {
            Ok(value) => value,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };

        if quantity <= 0.0 {
            self.status_message = "退货数量必须大于 0".into();
            return;
        }

        let Some(return_date) = parse_date(self.return_form.return_time.trim()) else {
            self.status_message = "退货日期格式不正确，请使用 YYYY-MM-DD 或 YYYY/M/D".into();
            return;
        };

        let existing_return = self.editing_return_id.and_then(|return_id| {
            self.data
                .returns
                .iter()
                .find(|record| record.id == return_id)
                .map(|record| {
                    (
                        record.id,
                        record.source_order_id,
                        record.quantity,
                        record.created_at.clone(),
                    )
                })
        });

        let Some(order_index) = self
            .data
            .orders
            .iter()
            .position(|order| order.id == order_id)
        else {
            self.status_message = "关联订单不存在".into();
            return;
        };

        if !return_date_is_valid_for_order(return_date, &self.data.orders[order_index].order_time) {
            self.status_message = "退货日期不能早于原订单日期".into();
            return;
        }

        let available = self.data.orders[order_index].current_outbound_quantity();
        let editing_same_order = existing_return
            .as_ref()
            .is_some_and(|(_, old_order_id, _, _)| *old_order_id == order_id);
        let allowed = if editing_same_order {
            available
                + existing_return
                    .as_ref()
                    .map_or(0.0, |(_, _, old_qty, _)| *old_qty)
        } else {
            available
        };
        if quantity > allowed {
            self.status_message = format!("退货数量不能超过当前可退数量 {allowed:.2}");
            return;
        }

        let (return_id, old_order_id, old_quantity, created_at) =
            existing_return.unwrap_or_else(|| (Uuid::new_v4(), order_id, 0.0, now_string()));

        let record = match self.build_return_record(return_id, order_id, quantity, created_at) {
            Ok(record) => record,
            Err(err) => {
                self.status_message = err;
                return;
            }
        };
        if let Err(err) = validate_non_negative_fields(&[
            ("目录价", record.catalog_price),
            ("销售总价", record.invoice_total),
            ("返现", record.cashback),
            ("成本折扣", record.cost_discount),
            ("成本单价", record.cost_unit_price),
            ("成本总价", record.cost_total),
            ("售价折扣", record.sale_discount),
            ("销售单价", record.invoice_unit_price),
        ]) {
            self.status_message = err;
            return;
        }

        if let Some(editing_id) = self.editing_return_id {
            if let Some(old_order) = self
                .data
                .orders
                .iter_mut()
                .find(|order| order.id == old_order_id)
            {
                old_order.returned_quantity = (old_order.returned_quantity - old_quantity).max(0.0);
            }
            if let Some(new_order) = self
                .data
                .orders
                .iter_mut()
                .find(|order| order.id == order_id)
            {
                new_order.returned_quantity += quantity;
            }
            if let Some(existing) = self
                .data
                .returns
                .iter_mut()
                .find(|record| record.id == editing_id)
            {
                *existing = record;
            } else {
                self.status_message = "要编辑的退货记录不存在".into();
                return;
            }
            self.return_form = ReturnForm::default();
            self.editing_return_id = None;
            self.save_data("退货记录已更新，并已回写订单出库数量");
            return;
        }

        self.data.orders[order_index].returned_quantity += quantity;
        self.data.returns.push(record);
        self.return_form = ReturnForm::default();
        self.save_data("退货已登记，并已回写订单出库数量");
    }

    fn build_return_record(
        &self,
        id: Uuid,
        source_order_id: Uuid,
        quantity: f64,
        created_at: String,
    ) -> Result<ReturnRecord, String> {
        Ok(ReturnRecord {
            id,
            source_order_id,
            product_id: self.return_form.product_id,
            product_name: self.return_form.product_name.trim().to_string(),
            item_no: self.return_form.item_no.trim().to_string(),
            return_time: normalize_date(&self.return_form.return_time)
                .unwrap_or_else(|| self.return_form.return_time.trim().to_string()),
            customer_unit: self.return_form.customer_unit.trim().to_string(),
            customer_name: self.return_form.customer_name.trim().to_string(),
            brand: self.return_form.brand.trim().to_string(),
            unit: self.return_form.unit.trim().to_string(),
            catalog_price: parse_f64(&self.return_form.catalog_price)?,
            quantity,
            invoice_total: parse_f64(&self.return_form.invoice_total)?,
            invoice_status: String::new(),
            is_shipped: true,
            cashback: parse_f64(&self.return_form.cashback)?,
            cost_discount: parse_f64(&self.return_form.cost_discount)?,
            cost_unit_price: parse_f64(&self.return_form.cost_unit_price)?,
            cost_total: parse_f64(&self.return_form.cost_total)?,
            sale_discount: parse_f64(&self.return_form.sale_discount)?,
            invoice_unit_price: parse_f64(&self.return_form.invoice_unit_price)?,
            gross_profit: parse_f64(&self.return_form.gross_profit)?,
            remark: self.return_form.remark.trim().to_string(),
            invoice_no: String::new(),
            is_paid: true,
            paid_time: normalized_date_text(&self.return_form.paid_time),
            created_at,
        })
    }

    fn recalculate_return_amounts(&mut self) {
        match calculate_form_amounts(
            &self.return_form.quantity,
            &self.return_form.invoice_unit_price,
            &self.return_form.cost_unit_price,
            &self.return_form.cashback,
        ) {
            Ok(amounts) => {
                self.return_form.invoice_total = format_money(amounts.invoice_total);
                self.return_form.cost_total = format_money(amounts.cost_total);
                self.return_form.gross_profit = format_money(amounts.gross_profit);
                self.status_message = "退货金额已重算".into();
            }
            Err(err) => {
                self.status_message = err;
            }
        }
    }

    fn edit_return(&mut self, return_id: Uuid) {
        let Some(record) = self.data.returns.iter().find(|item| item.id == return_id) else {
            self.status_message = "要编辑的退货记录不存在".into();
            return;
        };

        self.return_form = ReturnForm {
            source_order_id: Some(record.source_order_id),
            product_id: record.product_id,
            product_name: record.product_name.clone(),
            item_no: record.item_no.clone(),
            return_time: record.return_time.clone(),
            customer_unit: record.customer_unit.clone(),
            customer_name: record.customer_name.clone(),
            brand: record.brand.clone(),
            unit: record.unit.clone(),
            catalog_price: format_money(record.catalog_price),
            quantity: format_money(record.quantity),
            invoice_total: format_money(record.invoice_total),
            cashback: format_money(record.cashback),
            cost_discount: format_money(record.cost_discount),
            cost_unit_price: format_money(record.cost_unit_price),
            cost_total: format_money(record.cost_total),
            sale_discount: format_money(record.sale_discount),
            invoice_unit_price: format_money(record.invoice_unit_price),
            gross_profit: format_money(record.gross_profit),
            remark: record.remark.clone(),
            paid_time: record.paid_time.clone(),
        };
        self.editing_return_id = Some(record.id);
        self.page = Page::Returns;
        self.status_message = format!(
            "正在编辑退货记录“{} / {}”",
            record.product_name, record.customer_unit
        );
    }

    fn cancel_return_edit(&mut self) {
        self.return_form = ReturnForm::default();
        self.editing_return_id = None;
        self.status_message = "已取消退货编辑".into();
    }

    fn clear_return_form(&mut self) {
        self.return_form = ReturnForm::default();
        self.status_message = "退货表单已清空".into();
    }

    fn delete_return(&mut self, return_id: Uuid) {
        let Some(record) = self
            .data
            .returns
            .iter()
            .find(|record| record.id == return_id)
            .cloned()
        else {
            self.status_message = "要删除的退货记录不存在".into();
            return;
        };

        if let Some(order) = self
            .data
            .orders
            .iter_mut()
            .find(|order| order.id == record.source_order_id)
        {
            order.returned_quantity = (order.returned_quantity - record.quantity).max(0.0);
        }

        self.data.returns.retain(|record| record.id != return_id);
        if self.editing_return_id == Some(return_id) {
            self.cancel_return_edit();
        }
        self.save_data("退货记录已删除，并已回写订单出库数量");
    }

    fn apply_product_to_order_form(&mut self, product_id: Uuid) -> Option<String> {
        let product = self
            .data
            .products
            .iter()
            .find(|item| item.id == product_id)?
            .clone();

        self.order_form.product_id = Some(product.id);
        self.order_form.product_name = product.name.clone();
        self.order_form.brand = product.brand.clone();
        self.order_form.catalog_price = format_money(product.outbound_price);
        self.order_form.cost_unit_price = format_money(product.purchase_price);
        self.order_form.invoice_unit_price = format_money(product.outbound_price);

        if let Ok(amounts) = calculate_form_amounts(
            &self.order_form.quantity,
            &self.order_form.invoice_unit_price,
            &self.order_form.cost_unit_price,
            &self.order_form.cashback,
        ) {
            self.order_form.invoice_total = format_money(amounts.invoice_total);
            self.order_form.cost_total = format_money(amounts.cost_total);
            self.order_form.gross_profit = format_money(amounts.gross_profit);
        }

        Some(product.name)
    }

    fn prepare_order_from_product(&mut self, product_id: Uuid) {
        self.order_form = OrderForm::default();
        let Some(product_name) = self.apply_product_to_order_form(product_id) else {
            return;
        };
        self.page = Page::Orders;
        self.status_message = format!("已根据产品“{}”预填下单表单", product_name);
    }

    fn selected_order_product_label(&self) -> String {
        self.order_form
            .product_id
            .and_then(|id| {
                self.data
                    .products
                    .iter()
                    .find(|product| product.id == id)
                    .map(|product| product.name.clone())
            })
            .unwrap_or_else(|| "选择已登记产品".into())
    }

    fn prepare_return_from_order(&mut self, order_id: Uuid) -> bool {
        let Some(order) = self.data.orders.iter().find(|item| item.id == order_id) else {
            self.status_message = "关联订单不存在".into();
            return false;
        };

        let available = order.current_outbound_quantity();
        if available <= 0.0 {
            self.status_message = format!(
                "订单“{} / {}”已无可退数量",
                order.product_name, order.customer_unit
            );
            return false;
        }

        let return_quantity = available;
        let refund_amount = order.refund_amount_for(return_quantity);

        self.return_form = ReturnForm {
            source_order_id: Some(order.id),
            product_id: order.product_id,
            product_name: order.product_name.clone(),
            item_no: order.item_no.clone(),
            return_time: today_string(),
            customer_unit: order.customer_unit.clone(),
            customer_name: order.customer_name.clone(),
            brand: order.brand.clone(),
            unit: order.unit.clone(),
            catalog_price: format_money(order.catalog_price),
            quantity: format_money(return_quantity),
            invoice_total: format_money(refund_amount),
            cashback: format_money(order.cashback),
            cost_discount: format_money(order.cost_discount),
            cost_unit_price: format_money(order.cost_unit_price),
            cost_total: format_money(order.cost_total),
            sale_discount: format_money(order.sale_discount),
            invoice_unit_price: format_money(order.invoice_unit_price),
            gross_profit: format_money(order.gross_profit),
            remark: format!("关联订单 {}", order.id),
            paid_time: order.paid_time.clone(),
        };
        self.page = Page::Returns;
        self.status_message = format!(
            "已根据订单“{} / {}”预填退货表单",
            order.product_name, order.customer_unit
        );
        true
    }

    fn selected_return_order_label(&self) -> String {
        self.return_form
            .source_order_id
            .and_then(|id| {
                self.data
                    .orders
                    .iter()
                    .find(|order| order.id == id)
                    .map(|order| {
                        format!(
                            "{}{} | {} | {}",
                            order.product_name,
                            Self::item_no_suffix(&order.item_no),
                            order.customer_unit,
                            order.order_time
                        )
                    })
            })
            .unwrap_or_else(|| "选择关联订单".into())
    }

    fn fill_return_from_selected_order(&mut self, order_id: Uuid) {
        if !self.prepare_return_from_order(order_id) {
            return;
        }
        if let Ok(quantity) = parse_f64(&self.return_form.quantity) {
            let capped = quantity.clamp(0.0, 1.0);
            self.return_form.quantity = format_money(capped);
            self.return_form.invoice_total =
                if let Some(order) = self.data.orders.iter().find(|item| item.id == order_id) {
                    format_money(order.refund_amount_for(capped))
                } else {
                    "0.00".into()
                };
        }
    }

    fn today_metrics(&self) -> (f64, f64, f64, f64) {
        let today = Local::now().date_naive();
        let mut outbound_qty = 0.0;
        let mut outbound_amount = 0.0;
        let mut return_qty = 0.0;
        let mut return_amount = 0.0;

        for order in &self.data.orders {
            let date_text = if order.delivery_date.trim().is_empty() {
                &order.order_time
            } else {
                &order.delivery_date
            };
            if parse_date(date_text) == Some(today) {
                outbound_qty += order.current_outbound_quantity();
                outbound_amount += order.current_outbound_amount();
            }
        }

        for record in &self.data.returns {
            if parse_date(&record.return_time) == Some(today) {
                return_qty += record.quantity;
                return_amount += record.invoice_total;
            }
        }

        (outbound_qty, outbound_amount, return_qty, return_amount)
    }

    fn set_dashboard_range(&mut self, start: NaiveDate, end: NaiveDate) {
        self.dashboard_filter.start_date = start.format("%Y-%m-%d").to_string();
        self.dashboard_filter.end_date = end.format("%Y-%m-%d").to_string();
    }

    fn dashboard_date_range(&self) -> Result<(NaiveDate, NaiveDate), String> {
        let raw_start = parse_date(&self.dashboard_filter.start_date)
            .ok_or_else(|| "统计开始日期格式不正确，请使用 YYYY-MM-DD 或 YYYY/M/D".to_string())?;
        let raw_end = parse_date(&self.dashboard_filter.end_date)
            .ok_or_else(|| "统计结束日期格式不正确，请使用 YYYY-MM-DD 或 YYYY/M/D".to_string())?;

        let start = raw_start.min(raw_end);
        let end = raw_start.max(raw_end);
        let days = end.signed_duration_since(start).num_days() + 1;
        if days > 366 {
            return Err("统计范围不能超过 366 天".into());
        }

        Ok((start, end))
    }

    fn build_daily_series(&self) -> Result<Vec<DailyStat>, String> {
        let (mut date, finish) = self.dashboard_date_range()?;
        let mut series = Vec::new();

        while date <= finish {
            let outbound_amount = self
                .data
                .orders
                .iter()
                .filter(|order| {
                    let date_text = if order.delivery_date.trim().is_empty() {
                        &order.order_time
                    } else {
                        &order.delivery_date
                    };
                    parse_date(date_text) == Some(date)
                })
                .map(Order::current_outbound_amount)
                .sum::<f64>();
            let return_amount = self
                .data
                .returns
                .iter()
                .filter(|record| parse_date(&record.return_time) == Some(date))
                .map(|record| record.invoice_total)
                .sum::<f64>();

            series.push(DailyStat {
                label: date.format("%m-%d").to_string(),
                outbound_amount,
                return_amount,
            });
            date += Duration::days(1);
        }

        Ok(series)
    }

    fn total_order_amount(&self) -> f64 {
        self.data
            .orders
            .iter()
            .map(|order| order.invoice_total)
            .sum()
    }

    fn total_return_amount(&self) -> f64 {
        self.data
            .returns
            .iter()
            .map(|record| record.invoice_total)
            .sum()
    }

    fn matches_query(query: &str, fields: &[&str]) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }

        fields
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
    }

    fn item_no_suffix(item_no: &str) -> String {
        let item_no = item_no.trim();
        if item_no.is_empty() {
            String::new()
        } else {
            format!(" / 货号 {item_no}")
        }
    }

    fn render_nav(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("医疗产品进销存");
            ui.separator();

            for (page, label) in [
                (Page::Dashboard, "面板"),
                (Page::Products, "产品管理"),
                (Page::Orders, "下单管理"),
                (Page::Returns, "退货管理"),
            ] {
                let selected = self.page == page;
                if ui.selectable_label(selected, label).clicked() {
                    self.page = page;
                }
            }

            ui.separator();
            ui.label(format!("上次保存: {}", self.data.saved_at));
        });
    }

    fn render_status(&mut self, ui: &mut Ui) {
        Frame::group(ui.style())
            .fill(Color32::from_rgb(247, 250, 252))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(&self.status_message).color(Color32::from_rgb(40, 60, 85)),
                    );
                    if ui.button("手动备份").clicked() {
                        self.data.saved_at = now_string();
                        self.status_message = match self.storage.create_manual_backup(&self.data) {
                            Ok(path) => format!("手动备份已创建: {}", path.display()),
                            Err(err) => format!("手动备份失败: {err}"),
                        };
                    }
                    if ui.button("导出CSV模板").clicked() {
                        self.status_message = match self.storage.export_csv_templates() {
                            Ok(paths) => format!(
                                "CSV模板已导出，共 {} 个文件，目录: {}",
                                paths.len(),
                                self.storage.export_dir().display()
                            ),
                            Err(err) => format!("导出CSV模板失败: {err}"),
                        };
                    }
                    if ui.button("导出产品CSV").clicked() {
                        self.status_message =
                            match self.storage.export_products_csv(&self.data.products) {
                                Ok(path) => format!("产品CSV已导出: {}", path.display()),
                                Err(err) => format!("导出产品CSV失败: {err}"),
                            };
                    }
                    if ui.button("导出全部CSV").clicked() {
                        self.status_message = match self.storage.export_all_csv(&self.data) {
                            Ok(paths) => format!(
                                "全部CSV已导出，共 {} 个文件，目录: {}",
                                paths.len(),
                                self.storage.export_dir().display()
                            ),
                            Err(err) => format!("导出全部CSV失败: {err}"),
                        };
                    }
                    if ui.button("导出订单CSV").clicked() {
                        self.status_message =
                            match self.storage.export_orders_csv(&self.data.orders) {
                                Ok(path) => format!("订单CSV已导出: {}", path.display()),
                                Err(err) => format!("导出订单CSV失败: {err}"),
                            };
                    }
                    if ui.button("导出退货CSV").clicked() {
                        self.status_message =
                            match self.storage.export_returns_csv(&self.data.returns) {
                                Ok(path) => format!("退货CSV已导出: {}", path.display()),
                                Err(err) => format!("导出退货CSV失败: {err}"),
                            };
                    }
                });
                ui.small(format!(
                    "数据文件: {} | 备份目录: {} | 导出目录: {}",
                    self.storage.data_file().display(),
                    self.storage.backup_dir().display(),
                    self.storage.export_dir().display()
                ));
            });
    }

    fn render_dashboard(&mut self, ui: &mut Ui) {
        let (today_outbound_qty, today_outbound_amount, today_return_qty, today_return_amount) =
            self.today_metrics();
        let total_products = self.data.products.len();
        let total_orders = self.data.orders.len();
        let total_returns = self.data.returns.len();
        let total_order_amount = self.total_order_amount();
        let total_return_amount = self.total_return_amount();

        ui.horizontal(|ui| {
            ui.label("统计范围");
            ui.text_edit_singleline(&mut self.dashboard_filter.start_date);
            ui.label("到");
            ui.text_edit_singleline(&mut self.dashboard_filter.end_date);
            ui.small("格式 YYYY-MM-DD 或 YYYY/M/D");
        });
        ui.horizontal(|ui| {
            let today = Local::now().date_naive();
            if ui.button("今日").clicked() {
                self.set_dashboard_range(today, today);
            }
            if ui.button("近7天").clicked() {
                self.set_dashboard_range(today - Duration::days(6), today);
            }
            if ui.button("本月").clicked() {
                let start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                    .expect("first day of the current month should be valid");
                self.set_dashboard_range(start, today);
            }
        });
        let series_result = self.build_daily_series();
        if let Err(err) = &series_result {
            ui.colored_label(Color32::from_rgb(190, 40, 40), err);
        }
        ui.add_space(8.0);

        Grid::new("dashboard_cards")
            .num_columns(3)
            .spacing([14.0, 14.0])
            .show(ui, |ui| {
                self.metric_card(ui, "产品数", &total_products.to_string(), "已登记产品");
                self.metric_card(ui, "订单数", &total_orders.to_string(), "累计下单记录");
                self.metric_card(ui, "退货记录", &total_returns.to_string(), "累计退货记录");
                ui.end_row();
                self.metric_card(
                    ui,
                    "今日出库数量",
                    &format_money(today_outbound_qty),
                    "按出库日期统计",
                );
                self.metric_card(
                    ui,
                    "今日出库金额",
                    &format!("¥{}", format_money(today_outbound_amount)),
                    "当前有效出库金额",
                );
                self.metric_card(
                    ui,
                    "今日退货金额",
                    &format!("¥{}", format_money(today_return_amount)),
                    &format!("退货数量 {}", format_money(today_return_qty)),
                );
                ui.end_row();
                self.metric_card(
                    ui,
                    "累计下单金额",
                    &format!("¥{}", format_money(total_order_amount)),
                    "按销售总价汇总",
                );
                self.metric_card(
                    ui,
                    "累计退货金额",
                    &format!("¥{}", format_money(total_return_amount)),
                    "按退货记录汇总",
                );
                self.metric_card(ui, "出库单", "可生成", "订单列表支持文本版出库单导出");
            });

        ui.add_space(12.0);
        Frame::group(ui.style())
            .fill(Color32::WHITE)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("出库 / 退货折线");
                    ui.small("按日期统计当前有效出库金额与退货金额");
                });
                ui.add_space(8.0);
                let series = series_result.as_deref().unwrap_or(&[]);
                self.line_chart(ui, series);
            });
    }

    fn metric_card(&self, ui: &mut Ui, title: &str, value: &str, hint: &str) {
        Frame::group(ui.style())
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 226, 232)))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(260.0, 96.0));
                ui.label(
                    RichText::new(title)
                        .size(15.0)
                        .color(Color32::from_rgb(95, 110, 125)),
                );
                ui.add_space(8.0);
                ui.label(RichText::new(value).size(28.0).strong());
                ui.small(hint);
            });
    }

    fn line_chart(&self, ui: &mut Ui, series: &[DailyStat]) {
        let desired_size = Vec2::new(ui.available_width(), 260.0);
        let (response, painter) = ui.allocate_painter(desired_size, Sense::hover());
        let rect = response.rect;

        painter.rect_stroke(
            rect,
            8.0,
            Stroke::new(1.0, Color32::from_rgb(220, 226, 232)),
            egui::StrokeKind::Inside,
        );

        if series.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "暂无数据",
                FontId::proportional(18.0),
                Color32::GRAY,
            );
            return;
        }

        let left = rect.left() + 42.0;
        let right = rect.right() - 16.0;
        let top = rect.top() + 34.0;
        let bottom = rect.bottom() - 34.0;
        let chart_rect = egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom));
        let max_amount = series
            .iter()
            .map(|item| item.outbound_amount.max(item.return_amount))
            .fold(0.0, f64::max)
            .max(1.0);

        painter.circle_filled(
            egui::pos2(chart_rect.left(), rect.top() + 17.0),
            4.0,
            Color32::from_rgb(27, 116, 228),
        );
        painter.text(
            egui::pos2(chart_rect.left() + 10.0, rect.top() + 17.0),
            Align2::LEFT_CENTER,
            "出库金额",
            FontId::proportional(13.0),
            Color32::from_gray(90),
        );
        painter.circle_filled(
            egui::pos2(chart_rect.left() + 78.0, rect.top() + 17.0),
            4.0,
            Color32::from_rgb(225, 91, 57),
        );
        painter.text(
            egui::pos2(chart_rect.left() + 88.0, rect.top() + 17.0),
            Align2::LEFT_CENTER,
            "退货金额",
            FontId::proportional(13.0),
            Color32::from_gray(90),
        );

        painter.line_segment(
            [
                egui::pos2(chart_rect.left(), chart_rect.bottom()),
                egui::pos2(chart_rect.right(), chart_rect.bottom()),
            ],
            Stroke::new(1.0, Color32::from_gray(180)),
        );
        painter.line_segment(
            [
                egui::pos2(chart_rect.left(), chart_rect.top()),
                egui::pos2(chart_rect.left(), chart_rect.bottom()),
            ],
            Stroke::new(1.0, Color32::from_gray(180)),
        );

        for i in 0..=4 {
            let t = i as f32 / 4.0;
            let y = egui::lerp(chart_rect.bottom()..=chart_rect.top(), t);
            painter.line_segment(
                [
                    egui::pos2(chart_rect.left(), y),
                    egui::pos2(chart_rect.right(), y),
                ],
                Stroke::new(1.0, Color32::from_rgb(238, 242, 245)),
            );
            let value = max_amount * (i as f64 / 4.0);
            painter.text(
                egui::pos2(chart_rect.left() - 8.0, y),
                Align2::RIGHT_CENTER,
                format_money(value),
                FontId::proportional(12.0),
                Color32::from_gray(110),
            );
        }

        let count = series.len().max(2);
        let mut outbound_points = Vec::new();
        let mut return_points = Vec::new();
        for (idx, item) in series.iter().enumerate() {
            let x = egui::lerp(
                chart_rect.left()..=chart_rect.right(),
                idx as f32 / (count - 1) as f32,
            );
            let outbound_ratio = (item.outbound_amount / max_amount) as f32;
            let return_ratio = (item.return_amount / max_amount) as f32;
            let outbound_point = egui::pos2(
                x,
                egui::lerp(chart_rect.bottom()..=chart_rect.top(), outbound_ratio),
            );
            let return_point = egui::pos2(
                x,
                egui::lerp(chart_rect.bottom()..=chart_rect.top(), return_ratio),
            );
            outbound_points.push(outbound_point);
            return_points.push(return_point);

            painter.circle_filled(outbound_point, 3.5, Color32::from_rgb(27, 116, 228));
            painter.circle_filled(return_point, 3.5, Color32::from_rgb(225, 91, 57));
            painter.text(
                egui::pos2(x, chart_rect.bottom() + 10.0),
                Align2::CENTER_TOP,
                &item.label,
                FontId::proportional(12.0),
                Color32::from_gray(110),
            );
        }

        painter.add(egui::Shape::line(
            outbound_points,
            Stroke::new(2.5, Color32::from_rgb(27, 116, 228)),
        ));
        painter.add(egui::Shape::line(
            return_points,
            Stroke::new(2.0, Color32::from_rgb(225, 91, 57)),
        ));
    }

    fn render_products(&mut self, ui: &mut Ui) {
        ui.columns(2, |columns| {
            columns[0].heading(if self.editing_product_id.is_some() {
                "编辑产品"
            } else {
                "新增产品"
            });
            columns[0].add_space(8.0);
            form_text_row(&mut columns[0], "产品名称", &mut self.product_form.name);
            form_text_row(
                &mut columns[0],
                "进货金额",
                &mut self.product_form.purchase_price,
            );
            form_text_row(
                &mut columns[0],
                "出库金额",
                &mut self.product_form.outbound_price,
            );
            form_text_row(
                &mut columns[0],
                "规格/型号",
                &mut self.product_form.specification,
            );
            form_text_row(&mut columns[0], "品牌", &mut self.product_form.brand);
            form_text_row(
                &mut columns[0],
                "图片路径",
                &mut self.product_form.image_path,
            );
            columns[0].label("描述");
            columns[0].add(TextEdit::multiline(&mut self.product_form.description).desired_rows(5));
            columns[0].horizontal(|ui| {
                if ui.button("保存产品").clicked() {
                    self.save_product();
                }
                if self.editing_product_id.is_some() {
                    if ui.button("取消编辑").clicked() {
                        self.cancel_product_edit();
                    }
                } else if ui.button("清空表单").clicked() {
                    self.clear_product_form();
                }
            });
            columns[0].separator();
            columns[0].small("从按模板整理好的 CSV 导入产品基础资料。");
            columns[0].horizontal(|ui| {
                ui.label("CSV路径");
                ui.add(
                    TextEdit::singleline(&mut self.product_import_path)
                        .desired_width(f32::INFINITY)
                        .hint_text("例如 D:\\products.csv"),
                );
            });
            if columns[0].button("导入产品CSV").clicked() {
                self.import_products_csv();
            }

            columns[1].heading("产品列表");
            columns[1].horizontal(|ui| {
                ui.label("搜索");
                ui.add(
                    TextEdit::singleline(&mut self.product_query)
                        .desired_width(f32::INFINITY)
                        .hint_text("名称、品牌、规格"),
                );
                if !self.product_query.is_empty() && ui.button("清空").clicked() {
                    self.product_query.clear();
                }
            });
            columns[1].add_space(8.0);
            let products = sorted_products_for_display(&self.data.products)
                .into_iter()
                .filter(|product| {
                    Self::matches_query(
                        &self.product_query,
                        &[
                            &product.name,
                            &product.brand,
                            &product.specification,
                            &product.description,
                        ],
                    )
                })
                .collect::<Vec<_>>();
            columns[1].small(format!(
                "共 {} 条，当前显示 {} 条",
                self.data.products.len(),
                products.len()
            ));
            columns[1].add_space(6.0);
            ScrollArea::vertical().show(&mut columns[1], |ui| {
                for product in products {
                    Frame::group(ui.style())
                        .fill(Color32::WHITE)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.heading(&product.name);
                                ui.separator();
                                ui.label(format!("品牌: {}", product.brand));
                                ui.separator();
                                ui.label(format!("规格: {}", product.specification));
                            });
                            ui.label(format!(
                                "进货 ¥{} / 出库 ¥{}",
                                format_money(product.purchase_price),
                                format_money(product.outbound_price)
                            ));
                            if !product.image_path.is_empty() {
                                ui.small(format!("图片路径: {}", product.image_path));
                            }
                            if !product.description.is_empty() {
                                ui.small(format!("描述: {}", product.description));
                            }
                            ui.horizontal(|ui| {
                                if ui.button("基于该产品下单").clicked() {
                                    self.prepare_order_from_product(product.id);
                                }
                                if ui.button("编辑").clicked() {
                                    self.edit_product(product.id);
                                }
                                if ui.button("删除").clicked() {
                                    self.delete_product(product.id);
                                }
                                ui.small(format!("创建时间: {}", product.created_at));
                            });
                        });
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn render_orders(&mut self, ui: &mut Ui) {
        ui.columns(2, |columns| {
            columns[0].heading(if self.editing_order_id.is_some() {
                "编辑订单"
            } else {
                "新增订单"
            });
            columns[0].add_space(8.0);
            columns[0].label("选择已登记产品");
            ComboBox::from_id_salt("order_product_combo")
                .selected_text(self.selected_order_product_label())
                .show_ui(&mut columns[0], |ui| {
                    let product_options: Vec<(Uuid, String)> = self
                        .data
                        .products
                        .iter()
                        .map(|product| {
                            (
                                product.id,
                                format!(
                                    "{} | 品牌 {} | 出库 ¥{}",
                                    product.name,
                                    if product.brand.is_empty() {
                                        "-"
                                    } else {
                                        &product.brand
                                    },
                                    format_money(product.outbound_price)
                                ),
                            )
                        })
                        .collect();

                    if product_options.is_empty() {
                        ui.label("暂无已登记产品");
                    }
                    for (product_id, label) in product_options {
                        if ui.selectable_label(false, label).clicked() {
                            match self.apply_product_to_order_form(product_id) {
                                Some(product_name) => {
                                    self.status_message =
                                        format!("已选择产品“{}”，并预填价格", product_name);
                                }
                                None => {
                                    self.status_message = "选择的产品不存在".into();
                                }
                            }
                        }
                    }
                });
            columns[0].add_space(6.0);
            form_text_row(
                &mut columns[0],
                "产品名称",
                &mut self.order_form.product_name,
            );
            form_text_row(&mut columns[0], "货号", &mut self.order_form.item_no);
            form_text_row(&mut columns[0], "订购时间", &mut self.order_form.order_time);
            form_text_row(
                &mut columns[0],
                "出库日期",
                &mut self.order_form.delivery_date,
            );
            form_text_row(
                &mut columns[0],
                "订货单位",
                &mut self.order_form.customer_unit,
            );
            form_text_row(
                &mut columns[0],
                "订货人",
                &mut self.order_form.customer_name,
            );
            form_text_row(&mut columns[0], "品牌", &mut self.order_form.brand);
            form_text_row(&mut columns[0], "单位", &mut self.order_form.unit);
            form_text_row(
                &mut columns[0],
                "目录价",
                &mut self.order_form.catalog_price,
            );
            form_text_row(&mut columns[0], "数量", &mut self.order_form.quantity);
            form_text_row(
                &mut columns[0],
                "销售总价",
                &mut self.order_form.invoice_total,
            );
            form_text_row(&mut columns[0], "返现", &mut self.order_form.cashback);
            form_text_row(
                &mut columns[0],
                "成本折扣",
                &mut self.order_form.cost_discount,
            );
            form_text_row(
                &mut columns[0],
                "成本单价",
                &mut self.order_form.cost_unit_price,
            );
            form_text_row(&mut columns[0], "成本总价", &mut self.order_form.cost_total);
            form_text_row(
                &mut columns[0],
                "售价折扣",
                &mut self.order_form.sale_discount,
            );
            form_text_row(
                &mut columns[0],
                "销售单价",
                &mut self.order_form.invoice_unit_price,
            );
            form_text_row(&mut columns[0], "毛利", &mut self.order_form.gross_profit);
            columns[0].label("备注");
            columns[0].add(TextEdit::multiline(&mut self.order_form.remark).desired_rows(4));
            columns[0].horizontal(|ui| {
                if ui.button("重算金额").clicked() {
                    self.recalculate_order_amounts();
                }
                if ui.button("保存订单").clicked() {
                    self.save_order();
                }
                if self.editing_order_id.is_some() {
                    if ui.button("取消编辑").clicked() {
                        self.cancel_order_edit();
                    }
                } else if ui.button("清空表单").clicked() {
                    self.clear_order_form();
                }
            });
            columns[0].separator();
            columns[0].small("从按模板整理好的 CSV 导入订单记录。");
            columns[0].horizontal(|ui| {
                ui.label("CSV路径");
                ui.add(
                    TextEdit::singleline(&mut self.order_import_path)
                        .desired_width(f32::INFINITY)
                        .hint_text("例如 D:\\orders.csv"),
                );
            });
            if columns[0].button("导入订单CSV").clicked() {
                self.import_orders_csv();
            }
            columns[0].small("订单保存后，可在右侧列表生成文本版出库单。");

            columns[1].heading("订单列表");
            columns[1].horizontal(|ui| {
                ui.label("搜索");
                ui.add(
                    TextEdit::singleline(&mut self.order_query)
                        .desired_width(f32::INFINITY)
                        .hint_text("产品、货号、单位、订货人"),
                );
                if !self.order_query.is_empty() && ui.button("清空").clicked() {
                    self.order_query.clear();
                }
            });
            columns[1].add_space(8.0);
            let orders = sorted_orders_for_display(&self.data.orders)
                .into_iter()
                .filter(|order| {
                    Self::matches_query(
                        &self.order_query,
                        &[
                            &order.product_name,
                            &order.item_no,
                            &order.customer_unit,
                            &order.customer_name,
                            &order.remark,
                        ],
                    )
                })
                .collect::<Vec<_>>();
            columns[1].small(format!(
                "共 {} 条，当前显示 {} 条",
                self.data.orders.len(),
                orders.len()
            ));
            columns[1].add_space(6.0);
            ScrollArea::vertical().show(&mut columns[1], |ui| {
                for order in orders {
                    Frame::group(ui.style())
                        .fill(Color32::WHITE)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.heading(&order.product_name);
                                if !order.item_no.trim().is_empty() {
                                    ui.separator();
                                    ui.label(format!("货号: {}", order.item_no));
                                }
                                ui.separator();
                                ui.label(format!("单位: {}", order.customer_unit));
                                ui.separator();
                                ui.label(format!("订货人: {}", order.customer_name));
                            });
                            ui.label(format!(
                                "数量 {} {} | 当前出库 {} | 已退 {}",
                                format_money(order.quantity),
                                order.unit,
                                format_money(order.current_outbound_quantity()),
                                format_money(order.returned_quantity)
                            ));
                            ui.label(format!(
                                "销售总价 ¥{} | 当前出库金额 ¥{} | 毛利 ¥{}",
                                format_money(order.invoice_total),
                                format_money(order.current_outbound_amount()),
                                format_money(order.gross_profit)
                            ));
                            ui.small(format!(
                                "订购 {} | 出库 {}",
                                order.order_time,
                                if order.delivery_date.is_empty() {
                                    "-"
                                } else {
                                    &order.delivery_date
                                }
                            ));
                            if !order.remark.is_empty() {
                                ui.small(format!("备注: {}", order.remark));
                            }
                            ui.horizontal(|ui| {
                                if ui.button("登记退货").clicked() {
                                    self.prepare_return_from_order(order.id);
                                }
                                if ui.button("编辑").clicked() {
                                    self.edit_order(order.id);
                                }
                                if ui.button("删除").clicked() {
                                    self.delete_order(order.id);
                                }
                                if ui.button("生成出库单").clicked() {
                                    self.status_message =
                                        match self.storage.create_outbound_note(&order) {
                                            Ok(path) => {
                                                format!("出库单已生成: {}", path.display())
                                            }
                                            Err(err) => format!("生成出库单失败: {err}"),
                                        };
                                }
                            });
                        });
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn render_returns(&mut self, ui: &mut Ui) {
        ui.columns(2, |columns| {
            columns[0].heading(if self.editing_return_id.is_some() {
                "编辑退货"
            } else {
                "新增退货"
            });
            columns[0].add_space(8.0);

            columns[0].label("关联订单");
            ComboBox::from_id_salt("return_order_combo")
                .selected_text(self.selected_return_order_label())
                .show_ui(&mut columns[0], |ui| {
                    let order_options: Vec<(Uuid, String)> = self
                        .data
                        .orders
                        .iter()
                        .filter(|order| order.current_outbound_quantity() > 0.0)
                        .map(|order| {
                            (
                                order.id,
                                format!(
                                    "{}{} | {} | 可退 {}",
                                    order.product_name,
                                    Self::item_no_suffix(&order.item_no),
                                    order.customer_unit,
                                    format_money(order.current_outbound_quantity())
                                ),
                            )
                        })
                        .collect();

                    if order_options.is_empty() {
                        ui.label("暂无可退订单");
                    }
                    for (order_id, label) in order_options {
                        if ui.selectable_label(false, label).clicked() {
                            self.fill_return_from_selected_order(order_id);
                        }
                    }
                });

            form_text_row(
                &mut columns[0],
                "产品名称",
                &mut self.return_form.product_name,
            );
            form_text_row(&mut columns[0], "货号", &mut self.return_form.item_no);
            form_text_row(
                &mut columns[0],
                "退货日期",
                &mut self.return_form.return_time,
            );
            form_text_row(
                &mut columns[0],
                "订货单位",
                &mut self.return_form.customer_unit,
            );
            form_text_row(
                &mut columns[0],
                "订货人",
                &mut self.return_form.customer_name,
            );
            form_text_row(&mut columns[0], "品牌", &mut self.return_form.brand);
            form_text_row(&mut columns[0], "单位", &mut self.return_form.unit);
            form_text_row(
                &mut columns[0],
                "目录价",
                &mut self.return_form.catalog_price,
            );
            form_text_row(&mut columns[0], "数量", &mut self.return_form.quantity);
            form_text_row(
                &mut columns[0],
                "销售总价",
                &mut self.return_form.invoice_total,
            );
            form_text_row(&mut columns[0], "返现", &mut self.return_form.cashback);
            form_text_row(
                &mut columns[0],
                "成本折扣",
                &mut self.return_form.cost_discount,
            );
            form_text_row(
                &mut columns[0],
                "成本单价",
                &mut self.return_form.cost_unit_price,
            );
            form_text_row(
                &mut columns[0],
                "成本总价",
                &mut self.return_form.cost_total,
            );
            form_text_row(
                &mut columns[0],
                "售价折扣",
                &mut self.return_form.sale_discount,
            );
            form_text_row(
                &mut columns[0],
                "销售单价",
                &mut self.return_form.invoice_unit_price,
            );
            form_text_row(&mut columns[0], "毛利", &mut self.return_form.gross_profit);
            columns[0].label("备注");
            columns[0].add(TextEdit::multiline(&mut self.return_form.remark).desired_rows(4));
            columns[0].horizontal(|ui| {
                if ui.button("重算金额").clicked() {
                    self.recalculate_return_amounts();
                }
                if ui.button("保存退货").clicked() {
                    self.save_return();
                }
                if self.editing_return_id.is_some() {
                    if ui.button("取消编辑").clicked() {
                        self.cancel_return_edit();
                    }
                } else if ui.button("清空表单").clicked() {
                    self.clear_return_form();
                }
            });
            columns[0].separator();
            columns[0].small("从按模板整理好的 CSV 导入退货记录。");
            columns[0].horizontal(|ui| {
                ui.label("CSV路径");
                ui.add(
                    TextEdit::singleline(&mut self.return_import_path)
                        .desired_width(f32::INFINITY)
                        .hint_text("例如 D:\\returns.csv"),
                );
            });
            if columns[0].button("导入退货CSV").clicked() {
                self.import_returns_csv();
            }

            columns[1].heading("退货记录");
            columns[1].horizontal(|ui| {
                ui.label("搜索");
                ui.add(
                    TextEdit::singleline(&mut self.return_query)
                        .desired_width(f32::INFINITY)
                        .hint_text("产品、货号、单位、订货人"),
                );
                if !self.return_query.is_empty() && ui.button("清空").clicked() {
                    self.return_query.clear();
                }
            });
            columns[1].add_space(8.0);
            let returns = sorted_returns_for_display(&self.data.returns)
                .into_iter()
                .filter(|record| {
                    Self::matches_query(
                        &self.return_query,
                        &[
                            &record.product_name,
                            &record.item_no,
                            &record.customer_unit,
                            &record.customer_name,
                            &record.remark,
                        ],
                    )
                })
                .collect::<Vec<_>>();
            columns[1].small(format!(
                "共 {} 条，当前显示 {} 条",
                self.data.returns.len(),
                returns.len()
            ));
            columns[1].add_space(6.0);
            ScrollArea::vertical().show(&mut columns[1], |ui| {
                for record in returns {
                    Frame::group(ui.style())
                        .fill(Color32::WHITE)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.heading(&record.product_name);
                                if !record.item_no.trim().is_empty() {
                                    ui.separator();
                                    ui.label(format!("货号: {}", record.item_no));
                                }
                                ui.separator();
                                ui.label(format!("单位: {}", record.customer_unit));
                                ui.separator();
                                ui.label(format!("退货日期: {}", record.return_time));
                            });
                            ui.label(format!(
                                "数量 {} {} | 退货金额 ¥{}",
                                format_money(record.quantity),
                                record.unit,
                                format_money(record.invoice_total)
                            ));
                            ui.small(format!("关联订单: {}", record.source_order_id));
                            if !record.remark.is_empty() {
                                ui.small(format!("备注: {}", record.remark));
                            }
                            ui.horizontal(|ui| {
                                if ui.button("编辑").clicked() {
                                    self.edit_return(record.id);
                                }
                                if ui.button("删除").clicked() {
                                    self.delete_return(record.id);
                                }
                            });
                        });
                    ui.add_space(8.0);
                }
            });
        });
    }
}

impl eframe::App for MedicalInventoryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            self.render_nav(ui);
        });

        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            self.render_status(ui);
        });

        egui::CentralPanel::default()
            .frame(Frame::default().fill(Color32::from_rgb(243, 246, 249)))
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| match self.page {
                    Page::Dashboard => self.render_dashboard(ui),
                    Page::Products => self.render_products(ui),
                    Page::Orders => self.render_orders(ui),
                    Page::Returns => self.render_returns(ui),
                });
            });
    }
}

fn form_text_row(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(TextEdit::singleline(value).desired_width(f32::INFINITY));
    ui.add_space(6.0);
}

fn validate_non_negative_fields(fields: &[(&str, f64)]) -> Result<(), String> {
    for (label, value) in fields {
        if *value < 0.0 {
            return Err(format!("{label}不能小于 0"));
        }
    }
    Ok(())
}

fn normalized_date_text(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        today_string()
    } else {
        normalize_date(value).unwrap_or_else(|| value.to_string())
    }
}

#[derive(Debug, PartialEq)]
struct CalculatedAmounts {
    invoice_total: f64,
    cost_total: f64,
    gross_profit: f64,
}

fn calculate_form_amounts(
    quantity: &str,
    invoice_unit_price: &str,
    cost_unit_price: &str,
    cashback: &str,
) -> Result<CalculatedAmounts, String> {
    let quantity = parse_f64(quantity)?;
    if quantity <= 0.0 {
        return Err("数量必须大于 0".into());
    }

    let invoice_unit_price = parse_f64(invoice_unit_price)?;
    let cost_unit_price = parse_f64(cost_unit_price)?;
    let cashback = parse_f64(cashback)?;
    validate_non_negative_fields(&[
        ("销售单价", invoice_unit_price),
        ("成本单价", cost_unit_price),
        ("返现", cashback),
    ])?;

    let invoice_total = invoice_unit_price * quantity;
    let cost_total = cost_unit_price * quantity;
    Ok(CalculatedAmounts {
        invoice_total,
        cost_total,
        gross_profit: invoice_total - cost_total - cashback,
    })
}

fn sorted_orders_for_display(orders: &[Order]) -> Vec<Order> {
    let mut orders = orders.to_vec();
    orders.sort_by(|a, b| {
        b.order_time
            .cmp(&a.order_time)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    orders
}

fn sorted_products_for_display(products: &[Product]) -> Vec<Product> {
    let mut products = products.to_vec();
    products.sort_by(|a, b| {
        normalized_identity_part(&a.name)
            .cmp(&normalized_identity_part(&b.name))
            .then_with(|| {
                normalized_identity_part(&a.brand).cmp(&normalized_identity_part(&b.brand))
            })
            .then_with(|| {
                normalized_identity_part(&a.specification)
                    .cmp(&normalized_identity_part(&b.specification))
            })
    });
    products
}

fn sorted_returns_for_display(records: &[ReturnRecord]) -> Vec<ReturnRecord> {
    let mut records = records.to_vec();
    records.sort_by(|a, b| {
        b.return_time
            .cmp(&a.return_time)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    records
}

fn return_date_is_valid_for_order(return_date: NaiveDate, order_time: &str) -> bool {
    match parse_date(order_time) {
        Some(order_date) => return_date >= order_date,
        None => true,
    }
}

fn delivery_date_is_valid_for_order(order_date: NaiveDate, delivery_date: NaiveDate) -> bool {
    delivery_date >= order_date
}

fn product_identity_matches(
    product: &Product,
    name: &str,
    brand: &str,
    specification: &str,
) -> bool {
    normalized_identity_part(&product.name) == normalized_identity_part(name)
        && normalized_identity_part(&product.brand) == normalized_identity_part(brand)
        && normalized_identity_part(&product.specification)
            == normalized_identity_part(specification)
}

#[derive(Debug, PartialEq, Eq)]
struct ProductImportSummary {
    imported: usize,
    skipped_duplicates: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct OrderImportSummary {
    imported: usize,
    skipped_duplicates: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct ReturnImportSummary {
    imported: usize,
    skipped_duplicates: usize,
}

struct OrderIdentity<'a> {
    product_name: &'a str,
    item_no: &'a str,
    order_time: &'a str,
    delivery_date: &'a str,
    customer_unit: &'a str,
    customer_name: &'a str,
    quantity: f64,
    invoice_total: f64,
}

struct ReturnIdentity<'a> {
    source_order_id: Uuid,
    product_name: &'a str,
    item_no: &'a str,
    return_time: &'a str,
    customer_unit: &'a str,
    customer_name: &'a str,
    quantity: f64,
    invoice_total: f64,
}

fn merge_imported_products(
    existing_products: &mut Vec<Product>,
    imported_products: Vec<Product>,
) -> ProductImportSummary {
    let mut summary = ProductImportSummary {
        imported: 0,
        skipped_duplicates: 0,
    };

    for product in imported_products {
        let duplicate = existing_products.iter().any(|existing| {
            product_identity_matches(
                existing,
                &product.name,
                &product.brand,
                &product.specification,
            )
        });
        if duplicate {
            summary.skipped_duplicates += 1;
        } else {
            existing_products.push(product);
            summary.imported += 1;
        }
    }

    summary
}

fn order_identity_matches(order: &Order, identity: &OrderIdentity<'_>) -> bool {
    normalized_identity_part(&order.product_name) == normalized_identity_part(identity.product_name)
        && normalized_identity_part(&order.item_no) == normalized_identity_part(identity.item_no)
        && normalized_identity_part(&order.order_time)
            == normalized_identity_part(identity.order_time)
        && normalized_identity_part(&order.delivery_date)
            == normalized_identity_part(identity.delivery_date)
        && normalized_identity_part(&order.customer_unit)
            == normalized_identity_part(identity.customer_unit)
        && normalized_identity_part(&order.customer_name)
            == normalized_identity_part(identity.customer_name)
        && (order.quantity - identity.quantity).abs() < f64::EPSILON
        && (order.invoice_total - identity.invoice_total).abs() < f64::EPSILON
}

fn merge_imported_orders(
    existing_orders: &mut Vec<Order>,
    imported_orders: Vec<Order>,
) -> OrderImportSummary {
    let mut summary = OrderImportSummary {
        imported: 0,
        skipped_duplicates: 0,
    };

    for order in imported_orders {
        let identity = OrderIdentity {
            product_name: &order.product_name,
            item_no: &order.item_no,
            order_time: &order.order_time,
            delivery_date: &order.delivery_date,
            customer_unit: &order.customer_unit,
            customer_name: &order.customer_name,
            quantity: order.quantity,
            invoice_total: order.invoice_total,
        };
        let duplicate = existing_orders
            .iter()
            .any(|existing| order_identity_matches(existing, &identity));
        if duplicate {
            summary.skipped_duplicates += 1;
        } else {
            existing_orders.push(order);
            summary.imported += 1;
        }
    }

    summary
}

fn return_identity_matches(record: &ReturnRecord, identity: &ReturnIdentity<'_>) -> bool {
    record.source_order_id == identity.source_order_id
        && normalized_identity_part(&record.product_name)
            == normalized_identity_part(identity.product_name)
        && normalized_identity_part(&record.item_no) == normalized_identity_part(identity.item_no)
        && normalized_identity_part(&record.return_time)
            == normalized_identity_part(identity.return_time)
        && normalized_identity_part(&record.customer_unit)
            == normalized_identity_part(identity.customer_unit)
        && normalized_identity_part(&record.customer_name)
            == normalized_identity_part(identity.customer_name)
        && (record.quantity - identity.quantity).abs() < f64::EPSILON
        && (record.invoice_total - identity.invoice_total).abs() < f64::EPSILON
}

fn merge_imported_returns(
    existing_orders: &mut [Order],
    existing_returns: &mut Vec<ReturnRecord>,
    imported_returns: Vec<ReturnRecord>,
) -> Result<ReturnImportSummary, String> {
    let mut summary = ReturnImportSummary {
        imported: 0,
        skipped_duplicates: 0,
    };

    for record in imported_returns {
        let identity = ReturnIdentity {
            source_order_id: record.source_order_id,
            product_name: &record.product_name,
            item_no: &record.item_no,
            return_time: &record.return_time,
            customer_unit: &record.customer_unit,
            customer_name: &record.customer_name,
            quantity: record.quantity,
            invoice_total: record.invoice_total,
        };
        let duplicate = existing_returns
            .iter()
            .any(|existing| return_identity_matches(existing, &identity));
        if duplicate {
            summary.skipped_duplicates += 1;
        } else {
            existing_returns.push(record);
            summary.imported += 1;
        }
    }

    if summary.imported > 0 {
        recalculate_order_returned_quantities(existing_orders, existing_returns);
        if let Some(order) = existing_orders
            .iter()
            .find(|order| order.returned_quantity - order.quantity > f64::EPSILON)
        {
            return Err(format!(
                "退货数量不能超过订单“{} / {}”数量 {}",
                order.product_name,
                order.customer_unit,
                format_money(order.quantity)
            ));
        }
    }

    Ok(summary)
}

fn recalculate_order_returned_quantities(orders: &mut [Order], returns: &[ReturnRecord]) {
    for order in orders {
        order.returned_quantity = returns
            .iter()
            .filter(|record| record.source_order_id == order.id)
            .map(|record| record.quantity)
            .sum();
    }
}

fn normalized_identity_part(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn order_for_display(product_name: &str, order_time: &str, created_at: &str) -> Order {
        Order {
            id: Uuid::new_v4(),
            product_id: None,
            product_name: product_name.into(),
            item_no: String::new(),
            order_time: order_time.into(),
            delivery_date: order_time.into(),
            customer_unit: String::new(),
            customer_name: String::new(),
            brand: String::new(),
            unit: "件".into(),
            catalog_price: 0.0,
            quantity: 1.0,
            invoice_total: 0.0,
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
            paid_time: order_time.into(),
            returned_quantity: 0.0,
            created_at: created_at.into(),
        }
    }

    fn return_for_display(product_name: &str, return_time: &str, created_at: &str) -> ReturnRecord {
        ReturnRecord {
            id: Uuid::new_v4(),
            source_order_id: Uuid::new_v4(),
            product_id: None,
            product_name: product_name.into(),
            item_no: String::new(),
            return_time: return_time.into(),
            customer_unit: String::new(),
            customer_name: String::new(),
            brand: String::new(),
            unit: "件".into(),
            catalog_price: 0.0,
            quantity: 1.0,
            invoice_total: 0.0,
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
            paid_time: return_time.into(),
            created_at: created_at.into(),
        }
    }

    fn product_for_identity(name: &str, brand: &str, specification: &str) -> Product {
        Product {
            id: Uuid::new_v4(),
            name: name.into(),
            purchase_price: 0.0,
            outbound_price: 0.0,
            specification: specification.into(),
            brand: brand.into(),
            image_path: String::new(),
            description: String::new(),
            created_at: "2026-05-30 00:00:00".into(),
        }
    }

    #[test]
    fn non_negative_validation_reports_the_first_negative_field() {
        let err = validate_non_negative_fields(&[("目录价", 10.0), ("销售总价", -1.0)])
            .expect_err("negative amount should be rejected");

        assert_eq!(err, "销售总价不能小于 0");
    }

    #[test]
    fn calculate_form_amounts_updates_totals_and_profit() {
        let amounts = calculate_form_amounts("3", "120", "80", "15").unwrap();

        assert_eq!(
            amounts,
            CalculatedAmounts {
                invoice_total: 360.0,
                cost_total: 240.0,
                gross_profit: 105.0,
            }
        );
    }

    #[test]
    fn records_are_sorted_newest_first_for_display() {
        let orders = sorted_orders_for_display(&[
            order_for_display("旧订单", "2026-05-28", "2026-05-28 08:00:00"),
            order_for_display("新订单", "2026-05-30", "2026-05-30 08:00:00"),
        ]);
        let returns = sorted_returns_for_display(&[
            return_for_display("旧退货", "2026-05-27", "2026-05-27 08:00:00"),
            return_for_display("新退货", "2026-05-30", "2026-05-30 08:00:00"),
        ]);

        assert_eq!(orders[0].product_name, "新订单");
        assert_eq!(returns[0].product_name, "新退货");
    }

    #[test]
    fn products_are_sorted_by_name_brand_and_specification() {
        let products = sorted_products_for_display(&[
            product_for_identity("B产品", "品牌A", "规格1"),
            product_for_identity("A产品", "品牌B", "规格1"),
            product_for_identity("A产品", "品牌A", "规格2"),
            product_for_identity("A产品", "品牌A", "规格1"),
        ]);

        assert_eq!(products[0].brand, "品牌A");
        assert_eq!(products[0].specification, "规格1");
        assert_eq!(products[1].brand, "品牌A");
        assert_eq!(products[1].specification, "规格2");
        assert_eq!(products[2].brand, "品牌B");
        assert_eq!(products[3].name, "B产品");
    }

    #[test]
    fn product_identity_matching_ignores_surrounding_space_and_case() {
        let product = product_for_identity("Alpha", "BrandA", "Spec-1");

        assert!(product_identity_matches(
            &product, " alpha ", " branda ", " spec-1 "
        ));
        assert!(!product_identity_matches(
            &product, "Alpha", "BrandB", "Spec-1"
        ));
    }

    #[test]
    fn merge_imported_products_skips_duplicate_product_identity() {
        let mut existing = vec![product_for_identity("Alpha", "BrandA", "Spec-1")];
        let imported = vec![
            product_for_identity(" alpha ", " branda ", " spec-1 "),
            product_for_identity("Beta", "BrandB", "Spec-2"),
        ];

        let summary = merge_imported_products(&mut existing, imported);

        assert_eq!(
            summary,
            ProductImportSummary {
                imported: 1,
                skipped_duplicates: 1,
            }
        );
        assert_eq!(existing.len(), 2);
        assert_eq!(existing[1].name, "Beta");
    }

    #[test]
    fn merge_imported_orders_skips_duplicate_order_identity() {
        let mut existing = vec![order_for_display(
            "Alpha",
            "2026-05-30",
            "2026-05-30 08:00:00",
        )];
        let imported = vec![
            order_for_display(" alpha ", "2026-05-30", "2026-05-30 09:00:00"),
            order_for_display("Beta", "2026-05-31", "2026-05-31 08:00:00"),
        ];

        let summary = merge_imported_orders(&mut existing, imported);

        assert_eq!(
            summary,
            OrderImportSummary {
                imported: 1,
                skipped_duplicates: 1,
            }
        );
        assert_eq!(existing.len(), 2);
        assert_eq!(existing[1].product_name, "Beta");
    }

    #[test]
    fn merge_imported_returns_skips_duplicates_and_updates_order_quantity() {
        let mut order = order_for_display("Alpha", "2026-05-30", "2026-05-30 08:00:00");
        order.quantity = 3.0;
        order.returned_quantity = 5.0;
        let order_id = order.id;
        let mut existing_orders = vec![order];
        let mut existing_return = return_for_display("Alpha", "2026-06-01", "2026-06-01 08:00:00");
        existing_return.source_order_id = order_id;
        let mut duplicate_return = existing_return.clone();
        duplicate_return.created_at = "2026-06-01 09:00:00".into();
        let mut new_return = return_for_display("Alpha", "2026-06-02", "2026-06-02 08:00:00");
        new_return.source_order_id = order_id;
        let mut existing_returns = vec![existing_return];

        let summary = merge_imported_returns(
            &mut existing_orders,
            &mut existing_returns,
            vec![duplicate_return, new_return],
        )
        .unwrap();

        assert_eq!(
            summary,
            ReturnImportSummary {
                imported: 1,
                skipped_duplicates: 1,
            }
        );
        assert_eq!(existing_returns.len(), 2);
        assert_eq!(existing_orders[0].returned_quantity, 2.0);
    }

    #[test]
    fn merge_imported_returns_rejects_quantity_over_order_quantity() {
        let order = order_for_display("Alpha", "2026-05-30", "2026-05-30 08:00:00");
        let order_id = order.id;
        let mut existing_orders = vec![order];
        let mut imported_return = return_for_display("Alpha", "2026-06-01", "2026-06-01 08:00:00");
        imported_return.source_order_id = order_id;
        imported_return.quantity = 2.0;
        let mut existing_returns = Vec::new();

        let err = merge_imported_returns(
            &mut existing_orders,
            &mut existing_returns,
            vec![imported_return],
        )
        .unwrap_err();

        assert!(err.contains("退货数量不能超过订单"));
    }

    #[test]
    fn return_date_cannot_be_before_order_date() {
        let return_date = parse_date("2026-05-29").unwrap();

        assert!(!return_date_is_valid_for_order(return_date, "2026-05-30"));
        assert!(return_date_is_valid_for_order(return_date, "2026-05-29"));
        assert!(return_date_is_valid_for_order(return_date, "bad date"));
    }

    #[test]
    fn delivery_date_cannot_be_before_order_date() {
        let order_date = parse_date("2026-05-30").unwrap();

        assert!(!delivery_date_is_valid_for_order(
            order_date,
            parse_date("2026-05-29").unwrap()
        ));
        assert!(delivery_date_is_valid_for_order(
            order_date,
            parse_date("2026-05-30").unwrap()
        ));
    }
}
